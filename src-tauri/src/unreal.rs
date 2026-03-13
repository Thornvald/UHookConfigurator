use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize)]
pub struct EngineInfo {
    pub version: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub engine_association: String,
    pub ubt_path: Option<String>,
    pub hook_exists: bool,
}

pub fn get_engines() -> HashMap<String, EngineInfo> {
    let mut engines = HashMap::new();

    // 1. Scan HKLM (Standard installs)
    if let Ok(hklm) =
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\EpicGames\\UnrealEngine")
    {
        for name_res in hklm.enum_keys() {
            if let Ok(version) = name_res {
                if let Ok(ver_key) = hklm.open_subkey(&version) {
                    if let Ok(install_dir) = ver_key.get_value::<String, _>("InstalledDirectory") {
                        engines.insert(
                            version.clone(),
                            EngineInfo {
                                version: version.clone(),
                                path: install_dir,
                            },
                        );
                    }
                }
            }
        }
    }

    // 2. Scan HKCU (Source/Custom builds)
    if let Ok(hkcu) =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey("SOFTWARE\\Epic Games\\Unreal Engine\\Builds")
    {
        for val_res in hkcu.enum_values() {
            if let Ok((_uuid, install_dir)) = val_res {
                if let Some(_install_dir_str) = install_dir.to_string().into() {
                    // basic string extraction
                    // Value is a string path
                    let _path_str = install_dir.to_string(); // we'll need to parse this properly
                }
            }
        }
    }

    // Proper HKCU scan
    if let Ok(hkcu) =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey("SOFTWARE\\Epic Games\\Unreal Engine\\Builds")
    {
        for val_res in hkcu.enum_values() {
            if let Ok((uuid, _reg_val)) = val_res {
                // winreg returns REG_SZ as a string, let's just get it as string
                if let Ok(install_dir) = hkcu.get_value::<String, _>(&uuid) {
                    engines.insert(
                        uuid.clone(),
                        EngineInfo {
                            version: "Source Build".to_string(),
                            path: install_dir,
                        },
                    );
                }
            }
        }
    }

    // 3. Fallback: Parse LauncherInstalled.dat
    let program_data =
        std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let dat_path =
        PathBuf::from(program_data).join("Epic\\UnrealEngineLauncher\\LauncherInstalled.dat");

    if dat_path.exists() {
        if let Ok(content) = fs::read_to_string(dat_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(installations) = json["InstallationList"].as_array() {
                    for install in installations {
                        if let (Some(app_name), Some(install_loc)) = (
                            install["AppName"].as_str(),
                            install["InstallLocation"].as_str(),
                        ) {
                            if app_name.starts_with("UE_") {
                                let version = app_name.replace("UE_", "");
                                if !engines.contains_key(&version) {
                                    engines.insert(
                                        version.clone(),
                                        EngineInfo {
                                            version: version.clone(),
                                            path: install_loc.to_string(),
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    engines
}

pub fn parse_project(uproject_path: &str) -> Result<ProjectInfo, String> {
    let path = Path::new(uproject_path);
    if !path.exists() {
        return Err("Project file does not exist".into());
    }

    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

    // Allow trailing commas or other weirdness sometimes found in uproject files by using simple regex or relaxed JSON
    let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

    let engine_association = json["EngineAssociation"].as_str().unwrap_or("").to_string();

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let engines = get_engines();
    let mut ubt_path = None;

    if let Some(engine) = engines.get(&engine_association) {
        let mut ubt = PathBuf::from(&engine.path);
        ubt.push("Engine");
        ubt.push("Binaries");
        ubt.push("DotNET");
        ubt.push("UnrealBuildTool");
        ubt.push("UnrealBuildTool.exe");

        if ubt.exists() {
            ubt_path = Some(ubt.to_string_lossy().to_string());
        }
    }

    let mut hook_exists = false;
    if let Some(parent) = path.parent() {
        let hook_path = parent.join(".git").join("hooks").join("post-merge");
        if hook_path.exists() {
            hook_exists = true;
        }
    }

    Ok(ProjectInfo {
        name,
        path: uproject_path.to_string(),
        engine_association,
        ubt_path,
        hook_exists,
    })
}

pub fn install_hook(uproject_path: &str, project_name: &str, ubt_path: &str) -> Result<(), String> {
    let project_dir = Path::new(uproject_path)
        .parent()
        .ok_or("Invalid project path")?;
    let git_dir = project_dir.join(".git");

    if !git_dir.exists() {
        return Err("No .git directory found. Please initialize Git repository first.".into());
    }

    let hooks_dir = git_dir.join("hooks");
    if !hooks_dir.exists() {
        fs::create_dir_all(&hooks_dir).map_err(|e| e.to_string())?;
    }

    let post_merge_path = hooks_dir.join("post-merge");

    // Replace backslashes with forward slashes for the bash script
    let ubt_path_forward = ubt_path.replace("\\", "/");
    let uproject_path_forward = uproject_path.replace("\\", "/");

    let script_content = format!(
        r#"#!/bin/sh
# Check if any source files changed in the last pull
changed_files=$(git diff-tree -r --name-only --no-commit-id ORIG_HEAD HEAD)

if echo "$changed_files" | grep -qE 'Source/|\.uplugin|\.uproject'; then
    echo "Unreal Engine: Code changes detected. Rebuilding binaries..."
    
    # Setup logs folders
    mkdir -p build-logs/success
    mkdir -p build-logs/failed
    
    LOG_FILE="build-logs/temp_build_$$.log"
    
    # Run UBT
    "{}" {}Editor Win64 Development -project="{}" -waitmutex > "$LOG_FILE" 2>&1
    
    if [ $? -eq 0 ]; then
        echo "Build succeeded."
        mv "$LOG_FILE" "build-logs/success/build_$(date +%Y%m%d_%H%M%S).log"
    else
        echo "Build failed. Check build-logs/failed for details."
        mv "$LOG_FILE" "build-logs/failed/build_$(date +%Y%m%d_%H%M%S).log"
    fi
else
    echo "Unreal Engine: No code changes. Skipping build."
fi
"#,
        ubt_path_forward, project_name, uproject_path_forward
    );

    fs::write(&post_merge_path, script_content).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn remove_hook(uproject_path: &str) -> Result<(), String> {
    let project_dir = Path::new(uproject_path)
        .parent()
        .ok_or("Invalid project path")?;

    let hook_path = project_dir.join(".git").join("hooks").join("post-merge");

    if hook_path.exists() {
        fs::remove_file(&hook_path).map_err(|e| format!("Failed to remove hook: {}", e))?;
    }

    Ok(())
}

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
    pub build_target: Option<String>,
    pub hook_exists: bool,
}

fn collect_files_with_suffix(dir: &Path, suffix: &str, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_suffix(&path, suffix, files);
            continue;
        }

        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
        {
            files.push(path);
        }
    }
}

fn pick_preferred_name(mut candidates: Vec<String>, preferred_name: &str) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }

    candidates.sort_unstable();
    candidates.dedup();

    let preferred_editor_target = format!("{}Editor", preferred_name);

    if let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.as_str() == preferred_editor_target.as_str())
    {
        return Some(candidate.clone());
    }

    if let Some(candidate) = candidates.iter().find(|candidate| {
        candidate
            .strip_suffix("Editor")
            .is_some_and(|base_name| base_name == preferred_name)
    }) {
        return Some(candidate.clone());
    }

    if candidates.len() == 1 {
        return candidates.into_iter().next();
    }

    candidates.into_iter().next()
}

fn discover_editor_target_name(project_dir: &Path, preferred_name: &str) -> Option<String> {
    let source_dir = project_dir.join("Source");
    if !source_dir.exists() {
        return None;
    }

    let mut target_files = Vec::new();
    collect_files_with_suffix(&source_dir, ".Target.cs", &mut target_files);

    let mut editor_targets = Vec::new();
    for target_file in target_files {
        let file_name = match target_file.file_name().and_then(|name| name.to_str()) {
            Some(file_name) => file_name,
            None => continue,
        };

        let target_name = match file_name.strip_suffix(".Target.cs") {
            Some(target_name) => target_name,
            None => continue,
        };

        let is_editor_target = target_name.ends_with("Editor")
            || fs::read_to_string(&target_file)
                .map(|content| content.contains("TargetType.Editor"))
                .unwrap_or(false);

        if is_editor_target {
            editor_targets.push(target_name.to_string());
        }
    }

    if let Some(target_name) = pick_preferred_name(editor_targets, preferred_name) {
        return Some(target_name);
    }

    let mut build_files = Vec::new();
    collect_files_with_suffix(&source_dir, ".Build.cs", &mut build_files);

    let module_names = build_files
        .into_iter()
        .filter_map(|build_file| {
            build_file
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(".Build.cs"))
                .map(str::to_string)
        })
        .map(|module_name| format!("{}Editor", module_name))
        .collect();

    pick_preferred_name(module_names, preferred_name)
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

    let build_target = path
        .parent()
        .and_then(|project_dir| discover_editor_target_name(project_dir, &name));

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
        build_target,
        hook_exists,
    })
}

pub fn install_hook(uproject_path: &str) -> Result<(), String> {
    let project_info = parse_project(uproject_path)?;
    let path = Path::new(uproject_path);
    let project_dir = path.parent().ok_or("Invalid project path")?;
    let git_dir = project_dir.join(".git");
    let build_target = project_info.build_target.ok_or(
        "Could not detect an Unreal Editor target from Source/*.Target.cs or Source/*.Build.cs",
    )?;
    let ubt_path = project_info.ubt_path.ok_or_else(|| {
        format!(
            "Could not find UnrealBuildTool for engine version '{}'. Ensure the engine is installed and registered.",
            project_info.engine_association
        )
    })?;

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
    UPROJECT_PATH="{}"
    BUILD_TARGET="{}"

    if [ ! -f "$UPROJECT_PATH" ]; then
        UPROJECT_FILE=$(find . -maxdepth 1 -type f -name "*.uproject" -print -quit 2>/dev/null)
        if [ -n "$UPROJECT_FILE" ]; then
            if command -v cygpath >/dev/null 2>&1; then
                UPROJECT_PATH=$(cygpath -m "$UPROJECT_FILE")
            else
                UPROJECT_PATH="$UPROJECT_FILE"
            fi
        fi
    fi

    if [ ! -f "$UPROJECT_PATH" ]; then
        echo "Build failed. Could not locate the Unreal project file."
        exit 1
    fi

    TARGET_FILE=$(find Source -type f -name "$BUILD_TARGET.Target.cs" -print -quit 2>/dev/null)

    if [ -z "$TARGET_FILE" ]; then
        TARGET_FILE=$(find Source -type f -name "*.Target.cs" 2>/dev/null | while IFS= read -r candidate; do
            if printf '%s\n' "$candidate" | grep -q 'Editor\.Target\.cs$' || grep -q 'TargetType\.Editor' "$candidate"; then
                printf '%s\n' "$candidate"
                break
            fi
        done)
    fi

    if [ -n "$TARGET_FILE" ]; then
        BUILD_TARGET=$(basename "$TARGET_FILE" ".Target.cs")
    else
        BUILD_FILE=$(find Source -type f -name "*.Build.cs" -print -quit 2>/dev/null)
        if [ -n "$BUILD_FILE" ]; then
            BUILD_TARGET="$(basename "$BUILD_FILE" ".Build.cs")Editor"
        fi
    fi

    if [ -z "$BUILD_TARGET" ]; then
        echo "Build failed. Could not detect an Unreal Editor target."
        exit 1
    fi
    
    # Run UBT
    "{}" "$BUILD_TARGET" Win64 Development -project="$UPROJECT_PATH" -waitmutex > "$LOG_FILE" 2>&1
    
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
        uproject_path_forward, build_target, ubt_path_forward
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

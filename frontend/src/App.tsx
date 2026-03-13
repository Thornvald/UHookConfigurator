import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, GitBranch, CheckCircle2, AlertCircle, Cpu, Trash2 } from "lucide-react";
import Starfield from "./components/Starfield";
import "./index.css";

interface ProjectInfo {
  name: string;
  path: string;
  engine_association: string;
  ubt_path: string | null;
  build_target: string | null;
  hook_exists: boolean;
}

function App() {
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [installStatus, setInstallStatus] = useState<"idle" | "installing" | "success" | "error" | "removing" | "removed">("idle");

  useEffect(() => {
    // Trigger engine loading to pre-warm the cache essentially
    invoke("get_engines").catch(console.error);
  }, []);

  const selectProject = async () => {
    setError(null);
    setInstallStatus("idle");
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Unreal Project",
            extensions: ["uproject"],
          },
        ],
      });

      if (selected && typeof selected === "string") {
        const info: ProjectInfo = await invoke("parse_project", { path: selected });
        setProject(info);
        if (!info.ubt_path) {
          setError(`Could not find UnrealBuildTool for engine version '${info.engine_association}'. Ensure the engine is installed and registered.`);
        } else if (!info.build_target) {
          setError("Could not detect an Unreal Editor target. Make sure your project has a valid Source/*.Target.cs file.");
        }
      }
    } catch (err: any) {
      console.error(err);
      setError(err.toString());
    }
  };

  const installHook = async () => {
    if (!project || !project.ubt_path || !project.build_target) return;
    
    setInstallStatus("installing");
    setError(null);

    try {
      await invoke("install_hook", {
        uprojectPath: project.path,
      });
      setInstallStatus("success");
      setProject({ ...project, hook_exists: true });
    } catch (err: any) {
      console.error(err);
      setInstallStatus("error");
      setError(err.toString());
    }
  };

  const removeHook = async () => {
    if (!project) return;
    
    setInstallStatus("removing");
    setError(null);

    try {
      await invoke("remove_hook", { uprojectPath: project.path });
      setInstallStatus("removed");
      setProject({ ...project, hook_exists: false });
    } catch (err: any) {
      console.error(err);
      setInstallStatus("error");
      setError(err.toString());
    }
  };

  return (
    <div className="min-h-screen bg-[var(--color-bg)] text-[var(--color-bg-ink)] p-8 flex flex-col items-center justify-center relative overflow-hidden">
      
      {/* Animated Starfield */}
      <Starfield />
      
      <main className="relative z-10 flex w-full max-w-[760px] flex-col gap-8">
        <header className="flex w-full flex-col items-center text-center">
          <div className="mb-4 inline-flex items-center justify-center rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-2.5 shadow-[var(--shadow-main)]">
            <GitBranch className="h-7 w-7 text-[var(--color-accent-strong)]" />
          </div>
          <div className="w-full max-w-[500px] rounded-[24px] border border-white/8 bg-[linear-gradient(180deg,rgba(255,255,255,0.06),rgba(255,255,255,0.02))] px-6 py-5 shadow-[var(--shadow-main)] backdrop-blur-md">
            <h1 className="mb-2 text-center text-[2.4rem] font-bold tracking-tight text-[var(--color-accent-strong)] sm:text-[2.8rem]">UHook Configurator</h1>
            <p className="text-center text-[13px] leading-6 text-[var(--color-muted)] sm:text-sm">
              <span className="block">Automate background building by installing a smart</span>
              <span className="block">Git post-merge hook into your Unreal Engine project.</span>
            </p>
          </div>
        </header>

        <section className="bg-[var(--color-surface)] border border-[var(--color-border)] rounded-2xl p-6 shadow-[var(--shadow-main)] backdrop-blur-md flex flex-col gap-6 transition-all duration-300">
          
          {/* Project Selection */}
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-[var(--color-accent)] tracking-wider uppercase">Target Project</span>
              <button 
                onClick={selectProject}
                className="px-4 py-2 rounded-full bg-white/5 border border-white/10 hover:border-white/30 hover:bg-white/10 transition-all text-sm font-semibold flex items-center gap-2 cursor-pointer"
              >
                <Folder className="w-4 h-4" />
                Browse .uproject
              </button>
            </div>

            {!project ? (
              <div className="border border-dashed border-white/20 rounded-xl p-8 text-center bg-white/5 flex flex-col items-center justify-center gap-3">
                <Folder className="w-10 h-10 text-[var(--color-muted)] opacity-50" />
                <p className="text-[var(--color-muted)] text-sm">Select a .uproject file to continue</p>
              </div>
            ) : (
              <div className="border border-[var(--color-border)] rounded-xl p-4 bg-[var(--color-surface-alt)] flex flex-col gap-2">
                <div className="flex items-center justify-between">
                  <h3 className="font-bold text-lg text-white">{project.name}</h3>
                  <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-white/10 text-xs font-medium border border-white/5 text-white">
                    <Cpu className="w-3.5 h-3.5" />
                    UE {project.engine_association}
                  </div>
                </div>
                <p className="text-xs text-[var(--color-muted)] break-all truncate" title={project.path}>{project.path}</p>
                {project.build_target && (
                  <p className="text-xs text-[var(--color-muted)]">Build target: {project.build_target}</p>
                )}
                
                {project.hook_exists && (
                  <div className="mt-2 inline-flex items-center gap-1.5 text-xs font-medium text-emerald-400 bg-emerald-400/10 px-2.5 py-1 rounded-md border border-emerald-400/20 self-start">
                    <CheckCircle2 className="w-3.5 h-3.5" />
                    Hook currently installed
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Errors */}
          {error && (
            <div className="p-4 rounded-xl bg-red-500/10 border border-red-500/20 text-red-300 text-sm flex gap-3 items-start">
              <AlertCircle className="w-5 h-5 shrink-0 mt-0.5" />
              <p className="leading-relaxed">{error}</p>
            </div>
          )}

          {/* Action Area */}
          {project && project.ubt_path && project.build_target && (
            <div className="mt-2 flex flex-col gap-3">
              {!project.hook_exists ? (
                <button
                  onClick={installHook}
                  disabled={installStatus === "installing" || installStatus === "success"}
                  className={`
                    relative overflow-hidden w-full py-3.5 rounded-xl font-bold text-sm transition-all duration-300 flex items-center justify-center gap-2
                    ${installStatus === "success" 
                      ? "bg-emerald-500 text-black shadow-[0_0_30px_rgba(16,185,129,0.3)]" 
                      : "bg-gradient-to-br from-white to-[#bdbdbd] text-black hover:scale-[1.02] hover:shadow-[0_16px_30px_rgba(0,0,0,0.45)] disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100 disabled:hover:shadow-none"
                    }
                  `}
                >
                  {installStatus === "success" ? (
                    <>
                      <CheckCircle2 className="w-5 h-5" />
                      Hook Installed Successfully
                    </>
                  ) : installStatus === "installing" ? (
                    <div className="w-5 h-5 border-2 border-black border-t-transparent rounded-full animate-spin"></div>
                  ) : (
                    "Install Post-Merge Hook"
                  )}
                </button>
              ) : (
                <button
                  onClick={removeHook}
                  disabled={installStatus === "removing" || installStatus === "removed"}
                  className={`
                    relative overflow-hidden w-full py-3.5 rounded-xl font-bold text-sm transition-all duration-300 flex items-center justify-center gap-2
                    ${installStatus === "removed" 
                      ? "bg-transparent border border-white/20 text-[var(--color-muted)]" 
                      : "bg-red-500/20 border border-red-500/50 text-red-300 hover:bg-red-500/30 disabled:opacity-50 disabled:cursor-not-allowed"
                    }
                  `}
                >
                  {installStatus === "removed" ? (
                    <>
                      <CheckCircle2 className="w-5 h-5" />
                      Hook Removed
                    </>
                  ) : installStatus === "removing" ? (
                    <div className="w-5 h-5 border-2 border-red-400 border-t-transparent rounded-full animate-spin"></div>
                  ) : (
                    <>
                      <Trash2 className="w-4 h-4" />
                      Remove Existing Hook
                    </>
                  )}
                </button>
              )}
              
              {installStatus !== "success" && installStatus !== "removed" && (
                <p className="text-center text-[11px] text-[var(--color-muted)]">
                  Operates on .git/hooks/post-merge
                </p>
              )}
            </div>
          )}
        </section>
      </main>
    </div>
  );
}

export default App;

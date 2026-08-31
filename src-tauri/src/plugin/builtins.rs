// Built-in plugins shipped with AutoDev.
// These are available in the marketplace without any network access.

use super::manifest::{
    PluginCommands, PluginDependency, PluginManifest, PluginOsCommands, PluginType,
};

/// Return all 5 built-in plugin manifests.
pub fn all_builtins() -> Vec<PluginManifest> {
    vec![
        worker_backend(),
        worker_frontend(),
        worker_devops(),
        worker_qa(),
        worker_security(),
    ]
}

/// Get a built-in plugin by name.
pub fn get_builtin(name: &str) -> Option<PluginManifest> {
    all_builtins().into_iter().find(|p| p.name == name)
}

// ─── worker-backend ──────────────────────────────────────────────────────────

fn worker_backend() -> PluginManifest {
    PluginManifest {
        name:        "worker-backend".into(),
        version:     "1.0.0".into(),
        description: "Backend worker environment: Rust toolchain, Python + uv, Docker (optional)".into(),
        author:      "AIRS Foundry".into(),
        plugin_type: PluginType::WorkerSetup,
        os_support:  vec!["windows".into(), "linux".into(), "macos".into()],
        builtin:     true,
        icon:        "⚙️".into(),
        tags:        vec!["rust".into(), "python".into(), "backend".into(), "docker".into()],
        dependencies: vec![
            PluginDependency {
                name:        "rust".into(),
                version:     ">=1.70".into(),
                check_cmd:   Some("rustc --version".into()),
                install_cmd: Some("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".into()),
                optional:    false,
            },
            PluginDependency {
                name:        "python".into(),
                version:     ">=3.10".into(),
                check_cmd:   Some("python3 --version".into()),
                install_cmd: None, // OS-specific; guided in runner
                optional:    false,
            },
            PluginDependency {
                name:        "uv".into(),
                version:     ">=0.1".into(),
                check_cmd:   Some("uv --version".into()),
                install_cmd: Some("curl -LsSf https://astral.sh/uv/install.sh | sh".into()),
                optional:    false,
            },
            PluginDependency {
                name:        "docker".into(),
                version:     ">=24.0".into(),
                check_cmd:   Some("docker --version".into()),
                install_cmd: None, // Too complex to inline; guided
                optional:    true,
            },
        ],
        commands: PluginCommands {
            install: Some(PluginOsCommands {
                linux:   Some("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && curl -LsSf https://astral.sh/uv/install.sh | sh".into()),
                macos:   Some("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && curl -LsSf https://astral.sh/uv/install.sh | sh".into()),
                windows: Some("winget install Rustlang.Rustup; winget install astral-sh.uv".into()),
            }),
            verify:    Some("rustc --version && python3 --version && uv --version".into()),
            uninstall: Some("rustup self uninstall -y".into()),
            run:       None,
        },
        config: None,
    }
}

// ─── worker-frontend ─────────────────────────────────────────────────────────

fn worker_frontend() -> PluginManifest {
    PluginManifest {
        name:        "worker-frontend".into(),
        version:     "1.0.0".into(),
        description: "Frontend worker environment: Node.js, npm, and common build tools".into(),
        author:      "AIRS Foundry".into(),
        plugin_type: PluginType::WorkerSetup,
        os_support:  vec!["windows".into(), "linux".into(), "macos".into()],
        builtin:     true,
        icon:        "🌐".into(),
        tags:        vec!["node".into(), "npm".into(), "frontend".into(), "react".into()],
        dependencies: vec![
            PluginDependency {
                name:        "node".into(),
                version:     ">=18.0".into(),
                check_cmd:   Some("node --version".into()),
                install_cmd: Some("curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && apt-get install -y nodejs".into()),
                optional:    false,
            },
            PluginDependency {
                name:        "npm".into(),
                version:     ">=9.0".into(),
                check_cmd:   Some("npm --version".into()),
                install_cmd: None, // bundled with node
                optional:    false,
            },
        ],
        commands: PluginCommands {
            install: Some(PluginOsCommands {
                linux:   Some("curl -fsSL https://fnm.vercel.app/install | bash && fnm install 20 && fnm use 20".into()),
                macos:   Some("brew install node@20".into()),
                windows: Some("winget install OpenJS.NodeJS.LTS".into()),
            }),
            verify:    Some("node --version && npm --version".into()),
            uninstall: None,
            run:       None,
        },
        config: None,
    }
}

// ─── worker-devops ───────────────────────────────────────────────────────────

fn worker_devops() -> PluginManifest {
    PluginManifest {
        name:        "worker-devops".into(),
        version:     "1.0.0".into(),
        description: "DevOps worker environment: Docker, kubectl, Terraform, and CI/CD tools".into(),
        author:      "AIRS Foundry".into(),
        plugin_type: PluginType::WorkerSetup,
        os_support:  vec!["windows".into(), "linux".into(), "macos".into()],
        builtin:     true,
        icon:        "🚀".into(),
        tags:        vec!["docker".into(), "kubernetes".into(), "terraform".into(), "devops".into(), "ci".into()],
        dependencies: vec![
            PluginDependency {
                name:        "docker".into(),
                version:     ">=24.0".into(),
                check_cmd:   Some("docker --version".into()),
                install_cmd: None,
                optional:    false,
            },
            PluginDependency {
                name:        "kubectl".into(),
                version:     ">=1.28".into(),
                check_cmd:   Some("kubectl version --client".into()),
                install_cmd: Some("curl -LO https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl && chmod +x kubectl && mv kubectl /usr/local/bin/".into()),
                optional:    true,
            },
            PluginDependency {
                name:        "terraform".into(),
                version:     ">=1.5".into(),
                check_cmd:   Some("terraform --version".into()),
                install_cmd: None,
                optional:    true,
            },
        ],
        commands: PluginCommands {
            install: Some(PluginOsCommands {
                linux:   Some("apt-get update && apt-get install -y docker.io && systemctl enable docker".into()),
                macos:   Some("brew install --cask docker && brew install kubectl terraform".into()),
                windows: Some("winget install Docker.DockerDesktop; winget install Hashicorp.Terraform".into()),
            }),
            verify:    Some("docker --version && kubectl version --client 2>/dev/null || echo 'kubectl optional'".into()),
            uninstall: None,
            run:       None,
        },
        config: None,
    }
}

// ─── worker-qa ───────────────────────────────────────────────────────────────

fn worker_qa() -> PluginManifest {
    PluginManifest {
        name:        "worker-qa".into(),
        version:     "1.0.0".into(),
        description: "QA worker environment: Python, pytest, playwright, and testing tools".into(),
        author:      "AIRS Foundry".into(),
        plugin_type: PluginType::WorkerSetup,
        os_support:  vec!["windows".into(), "linux".into(), "macos".into()],
        builtin:     true,
        icon:        "🧪".into(),
        tags:        vec!["testing".into(), "pytest".into(), "playwright".into(), "qa".into()],
        dependencies: vec![
            PluginDependency {
                name:        "python".into(),
                version:     ">=3.10".into(),
                check_cmd:   Some("python3 --version".into()),
                install_cmd: None,
                optional:    false,
            },
            PluginDependency {
                name:        "uv".into(),
                version:     ">=0.1".into(),
                check_cmd:   Some("uv --version".into()),
                install_cmd: Some("curl -LsSf https://astral.sh/uv/install.sh | sh".into()),
                optional:    false,
            },
        ],
        commands: PluginCommands {
            install: Some(PluginOsCommands {
                linux:   Some("curl -LsSf https://astral.sh/uv/install.sh | sh && uv pip install pytest pytest-asyncio playwright && playwright install --with-deps chromium".into()),
                macos:   Some("curl -LsSf https://astral.sh/uv/install.sh | sh && uv pip install pytest pytest-asyncio playwright && playwright install chromium".into()),
                windows: Some("winget install astral-sh.uv; uv pip install pytest pytest-asyncio playwright; playwright install chromium".into()),
            }),
            verify:    Some("python3 -m pytest --version && playwright --version".into()),
            uninstall: None,
            run:       None,
        },
        config: None,
    }
}

// ─── worker-security ─────────────────────────────────────────────────────────

fn worker_security() -> PluginManifest {
    PluginManifest {
        name: "worker-security".into(),
        version: "1.0.0".into(),
        description:
            "Security worker environment: static analysis, secret scanning, and CVE tooling".into(),
        author: "AIRS Foundry".into(),
        plugin_type: PluginType::Security,
        os_support: vec!["windows".into(), "linux".into(), "macos".into()],
        builtin: true,
        icon: "🔒".into(),
        tags: vec![
            "security".into(),
            "sast".into(),
            "secrets".into(),
            "audit".into(),
        ],
        dependencies: vec![
            PluginDependency {
                name: "python".into(),
                version: ">=3.10".into(),
                check_cmd: Some("python3 --version".into()),
                install_cmd: None,
                optional: false,
            },
            PluginDependency {
                name: "git".into(),
                version: ">=2.0".into(),
                check_cmd: Some("git --version".into()),
                install_cmd: None,
                optional: false,
            },
        ],
        commands: PluginCommands {
            install: Some(PluginOsCommands {
                linux: Some(
                    "uv pip install bandit trufflehog semgrep && cargo install cargo-audit".into(),
                ),
                macos: Some(
                    "uv pip install bandit trufflehog semgrep && cargo install cargo-audit".into(),
                ),
                windows: Some(
                    "uv pip install bandit trufflehog semgrep; cargo install cargo-audit".into(),
                ),
            }),
            verify: Some("bandit --version && semgrep --version".into()),
            uninstall: None,
            run: None,
        },
        config: None,
    }
}

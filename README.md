# Wolf Engine

	Wolf Engine is a modular Rust rendering engine with a pluggable backend system.
	By default, it uses the Vulkan backend.

	Workspace Structure
		engine – main application (wolf-engine crate)
		common – shared utilities and optional dependencies
		crates/core/renderer/api – renderer API abstractions
		crates/core/renderer/backend/vulkan – Vulkan backend implementation

	Running the Engine

		1. Standard Profiles

			Debug (default, unoptimized, full debug info):
				cargo run -p wolf-engine

			Release (optimized, minimal debug info):
				cargo run -p wolf-engine --release

		2. Custom Release Profiles

			We provide extra profiles for debugging optimized builds.
			Aliases are defined in .cargo/config.toml for convenience.

			Release with debug info (level 1):
				cargo release-with-debug
				(expands to: cargo run --profile release-with-debug -p wolf-engine)

			Release with full debug info (level 2):
				cargo release-with-full-debug
				(expands to: cargo run --profile release-with-full-debug -p wolf-engine)

		3. Feature Control (Backends)

			By default, the engine enables the vulkan feature.

			Run with Vulkan backend (default):
				cargo run -p wolf-engine
				or explicitly:
				cargo run -p wolf-engine --features vulkan

			Run with no backend:
				cargo run -p wolf-engine --no-default-features
				(this will trigger a compile error, since no renderer is selected)

			When additional backends are added (e.g., metal, opengl),
			you will be able to switch like this:
				cargo run -p wolf-engine --no-default-features --features metal

		4. Build Without Running

			Debug build only:
				cargo build -p wolf-engine

			Release build only:
				cargo build -p wolf-engine --release

			Custom profile build only:
				cargo build --profile release-with-debug -p wolf-engine

	Developer Notes
		Profiles are defined in the root Cargo.toml under [profile.*].
		Aliases are defined in .cargo/config.toml.
		Default features = Vulkan backend. Use --no-default-features to opt out.
		Future backends will follow the same pattern with their own features.

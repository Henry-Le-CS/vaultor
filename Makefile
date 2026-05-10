APP_NAME   := Vaultor
APP_DIR    := apps/vaultor
APP_BUNDLE := $(APP_DIR)/src-tauri/target/release/bundle/macos/$(APP_NAME).app

.PHONY: build open clean dev

## Build the .app bundle (production)
build:
	cd $(APP_DIR) && npm run tauri build

dev:
	cd $(APP_DIR) && npm run tauri dev

## Open the built .app
open: $(APP_BUNDLE)
	open $(APP_BUNDLE)

## Remove Rust build artifacts
clean:
	cargo clean --manifest-path $(APP_DIR)/src-tauri/Cargo.toml

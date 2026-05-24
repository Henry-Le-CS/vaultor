APP_NAME    := Vaultor
APP_DIR     := apps/vaultor
APP_BUNDLE  := $(APP_DIR)/src-tauri/target/release/bundle/macos/$(APP_NAME).app
INSTALL_DIR := /Applications

.PHONY: build install uninstall open clean dev

## Build the .app bundle (production)
build:
	cd $(APP_DIR) && npm run tauri build

## Copy the built .app into /Applications (build first if needed)
install: $(APP_BUNDLE)
	@echo "Installing $(APP_NAME).app → $(INSTALL_DIR)/$(APP_NAME).app"
	@rm -rf "$(INSTALL_DIR)/$(APP_NAME).app"
	@cp -r "$(APP_BUNDLE)" "$(INSTALL_DIR)/$(APP_NAME).app"
	@echo "Done. Launch with: open $(INSTALL_DIR)/$(APP_NAME).app"

## Uninstall Vaultor (interactive — prompts for confirmation)
uninstall:
	@bash uninstall.sh

## Build then install in one step
build-install: build install

dev:
	cd $(APP_DIR) && npm run tauri dev

## Open the built .app
open: $(APP_BUNDLE)
	open $(APP_BUNDLE)

## Remove Rust build artifacts
clean:
	cargo clean --manifest-path $(APP_DIR)/src-tauri/Cargo.toml

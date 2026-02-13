# Roadmap

High-level direction for Immerse Yourself. This is a living document.

## Recently Completed

### Audio License Compliance (Done)
- [x] Freesound license audit: 297 sounds audited (156 CC0, 98 CC-BY, 43 CC-BY-NC)
- [x] CC-BY-NC resolution: kept as URL references (runtime download, no redistribution)
- [x] Copyrighted sound configs removed (doh, omg, science_yes)
- [ ] CC-BY attribution placement (pending — 98 sounds need attribution)

### Security & Open Source Preparation (Done)
- [x] Gitleaks security scan: both repos clean, zero secrets in 227 commits
- [x] Open-core boundary defined: 20 free environments, 6 premium packs
- [ ] Contributor guidelines and CLA setup
- [ ] Documentation improvements

### User Content Directory (Done)
- [x] Platform-standard user content directory for custom configs, sounds, and sound collections
- [x] Multi-directory ConfigLoader with override-by-filename semantics
- [x] Settings UI panel with "Open Folder" button
- [x] Auto-created directory structure with README on first launch

## Current Focus

### Freesound API Licensing
- Investigate whether redistributing downloaded freesound audio requires separate API licensing from Freesound (distinct from individual CC licenses)

### Business Foundation
- LLC formation for liability protection
- Trademark filing for "Immerse Yourself"
- CONTRIBUTING.md + CLA setup

## Near Term

### Premium Content Packs
- Themed environment bundles (curated configs + audio)
- Distribution via Gumroad and itch.io
- License key validation for premium content

### Community
- Discord server for users and contributors
- Knowledge base and setup documentation
- CONTRIBUTING.md with "good first issue" workflow

## Future

### VTT Integration
- Foundry VTT module for audio/atmosphere integration
- Investigate Roll20 integration possibilities

### Analytics
- Opt-in, privacy-respecting usage analytics
- GDPR-compliant with self-hosting option

### Platform Expansion
- iOS app (in progress via Tauri)
- Broader smart light brand support beyond WIZ bulbs

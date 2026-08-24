# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/thoroc/street-view-movie-maker/releases/tag/v0.1.0) - 2026-08-24

### Added

- *(release)* add release-plz + cross-platform release pipeline ([#11](https://github.com/thoroc/street-view-movie-maker/pull/11))
- composite a moving-marker route map into the video corner ([#9](https://github.com/thoroc/street-view-movie-maker/pull/9))
- *(context)* add ready status to the plan lifecycle ([#8](https://github.com/thoroc/street-view-movie-maker/pull/8))
- *(routing)* add --avoid-tolls/--avoid-highways/--avoid-ferries
- *(main)* add --interactive/-i flag to prompt for unset values
- *(main)* show an estimated cost before the download confirmation gate
- *(main)* wire the full canonical pipeline end to end
- *(video)* ffmpeg encoding with parity flags and a startup check
- *(lineup)* content-hash dedupe and sequential renumbering
- *(itinerary)* dedupe, turn-frame insertion, and resumable persistence
- *(streetview)* metadata probing, image download, bounded concurrency
- *(directions)* call the Directions API and parse routes
- *(geo)* implement bearing/haversine/interpolation with Python-parity tests
- *(main)* scaffold CLI with usage-rs derive and API key validation

### Fixed

- *(release)* disable crates.io publish and always-release ([#13](https://github.com/thoroc/street-view-movie-maker/pull/13))
- *(hk)* actually wire the context-filenames check into pre-commit ([#6](https://github.com/thoroc/street-view-movie-maker/pull/6))
- *(hk)* auto-regenerate context index; add ctxharness to pre-push ([#3](https://github.com/thoroc/street-view-movie-maker/pull/3))

### Other

- wire in the plain-english instruction for human-facing output ([#10](https://github.com/thoroc/street-view-movie-maker/pull/10))
- add a real run screenshot and a reusable demo route task ([#7](https://github.com/thoroc/street-view-movie-maker/pull/7))
- cache cargo registry, target dir, and CI-tool crates ([#5](https://github.com/thoroc/street-view-movie-maker/pull/5))
- keep finding status in sync with implementation ([#4](https://github.com/thoroc/street-view-movie-maker/pull/4))
- *(ci)* adapt copied GitHub Actions workflows to this Rust CLI repo ([#2](https://github.com/thoroc/street-view-movie-maker/pull/2))
- *(hk)* add markdownlint check to pre-commit and fix gate ([#1](https://github.com/thoroc/street-view-movie-maker/pull/1))
- capture ADRs for the Python-to-Rust port and open the risk register
- *(skills)* finish removing external-project references from .claude/
- *(skills)* finalize imported Claude Code skills/instructions for this repo
- remove legacy Python implementation and demo artifacts
- clean up readme and standardise getting-started doc casing
- store encrypted API keys in fnox.toml
- replace plaintext .env with fnox for API key storage
- document the Rust CLI as the maintained entry point
- *(net)* extract shared retry-with-backoff helper
- set up mise and hk for dev tooling and git hooks
- Added files related to Hollerado project.
- Added lots of new functions, including create_itinerary_df (to manage a complex project), pruning images, lining up files. Added pandas to requirements.
- New plan to separate list-of-things-to-download (and save it) and downloading steps
- Added function to download images in a circle to effect a turn
- Added picsize (dimension of downloaded image) to input variables for relevant functions. Added util functions to download multiple images in a field of view and assemble grid into composite, with settable crop margin.
- Tidied up video preview
- Ignoring sandbox
- Removed cruft
- Updated readme and added preview image
- Added requirements file
- Basic readme added
- Fixed API key loading
- Added copacabana demo video and updated .gitignore.
- Moved remaining functions to utils. Created download_images_for_path and line_up_files functions. Added small radius to street view downloader to hopefully get images on a narrower path. Fleshed out main() callable function.
- Reorganizing project into standard func/utils/sandbox partition.
- Belated commit. Recent change: borrowed calculate_initial_compass_bearing gist from github to solve heading problem. Old changes: downloading metadata along with streetview image, traces of a few practice runs, scripts to download images along a path and convert using ffmpeg to a video
- Solved orientation issue by using distance computer that allows bearing as input (but doesnt estimate bearing... so I use a gross hack to locate optimal angle) and it works. Also, realized that computing angle between points a few steps away would naturally curve camera in anticipation of a turn.
- Added interpolation-by-distance: set a hop size and string points between A and B. Added directions API key (in previous commit actually) and use it to chart a route from home to Ushiku. Headings aren't being computed correctly though, based on that test.
- Attempted a longer example along santa monica in LA, realized that repeated images and stray incorrect images (at corners, even inside buildings) gets in the way, so worked on some ways to detect outliers
- Added code to transition between two frames (zooming in and superposing with alpha) and for finding the optimal way to superpose them, but that part actually seems not to work.
- Added routines to find heading between GPS points, and to linearly interpolate GPS points (and confirmed that interpolation can get us more unique points than relying on built-in snap_to_roads routine)
- Initial commit of street-view-photo-saving routine borrowed from the web

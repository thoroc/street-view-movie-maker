# Changelog

## [0.5.0](https://github.com/thoroc/street-view-movie-maker/compare/v0.4.0...v0.5.0) (2026-08-26)


### Features

* add turn-ahead road-sign overlay ([#33](https://github.com/thoroc/street-view-movie-maker/issues/33)) ([7eef1ad](https://github.com/thoroc/street-view-movie-maker/commit/7eef1adcc159c7c8d709c0adf8357b959beb5c22))
* filter off-route Street View frames and support manual exclusion ([#36](https://github.com/thoroc/street-view-movie-maker/issues/36)) ([2d00a5e](https://github.com/thoroc/street-view-movie-maker/commit/2d00a5ef113e8ad052ebf3d6ba629778151c48b8))

## [0.4.0](https://github.com/thoroc/street-view-movie-maker/compare/v0.3.0...v0.4.0) (2026-08-25)


### ⚠ BREAKING CHANGES

* the default output codec changes from ffmpeg's typical H.264 to AV1. AV1 is not natively playable on macOS pre-M3, stock Windows, or most Linux desktops without an extension.

### Features

* **context-index:** surface repo-scouting's log.jsonl in index.yaml ([#29](https://github.com/thoroc/street-view-movie-maker/issues/29)) ([9edc99d](https://github.com/thoroc/street-view-movie-maker/commit/9edc99d0b87817e1900c716704abb455f09a73f6))
* **release:** add release-plz + cross-platform release pipeline ([#11](https://github.com/thoroc/street-view-movie-maker/issues/11)) ([f1f18c0](https://github.com/thoroc/street-view-movie-maker/commit/f1f18c0a3356c56cc6510d64a32c271df5e7b5ad))
* **release:** give major releases a city codename ([#19](https://github.com/thoroc/street-view-movie-maker/issues/19)) ([6074c21](https://github.com/thoroc/street-view-movie-maker/commit/6074c213eabc0a6de640e66b332474b277f52b85))
* replace ffmpeg with a native Rust encode pipeline ([#20](https://github.com/thoroc/street-view-movie-maker/issues/20)) ([1172565](https://github.com/thoroc/street-view-movie-maker/commit/1172565b4da6f3a43a75564352456af55782d727))
* **skills:** add repo-scouting log skill ([#28](https://github.com/thoroc/street-view-movie-maker/issues/28)) ([b41fc78](https://github.com/thoroc/street-view-movie-maker/commit/b41fc7868f1d14390cc2bc4a97c1f3955017ce0b))


### Bug Fixes

* **hk:** exclude CHANGELOG.md from markdownlint ([#18](https://github.com/thoroc/street-view-movie-maker/issues/18)) ([a1e9c66](https://github.com/thoroc/street-view-movie-maker/commit/a1e9c663ae6f233c94237ad4a27fd6ef5cde4d73))
* **release:** disable crates.io publish and always-release ([#13](https://github.com/thoroc/street-view-movie-maker/issues/13)) ([8fda4c1](https://github.com/thoroc/street-view-movie-maker/commit/8fda4c1227514dd427eabde8b412cbba5c897043))
* **release:** keep breaking changes on a minor bump while pre-1.0 ([#21](https://github.com/thoroc/street-view-movie-maker/issues/21)) ([67b2954](https://github.com/thoroc/street-view-movie-maker/commit/67b295427039ad704c9edb3037b6c3a3abe77962))
* **release:** scope mise setup to rust only in release.yml ([#14](https://github.com/thoroc/street-view-movie-maker/issues/14)) ([8d27eb6](https://github.com/thoroc/street-view-movie-maker/commit/8d27eb62fed5eaf08359a628406e23738d450f71))
* **release:** switch from release-plz to release-please ([#16](https://github.com/thoroc/street-view-movie-maker/issues/16)) ([7d66786](https://github.com/thoroc/street-view-movie-maker/commit/7d6678609ca1e7a2e02327b831ea7311f5977dd0))

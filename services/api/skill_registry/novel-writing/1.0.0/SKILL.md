---
name: novel-writing
upstream: https://github.com/wgwtest/novel-writing
license: MIT
runtime-policy: mvp-a-structured-planning-only
---

# Novel Writing Runtime Policy

Use narrative causality, character knowledge, agency, continuity, and scene structure only as
planning constraints for StorySpec and ScriptSpec fields. SourceMaterial may be a novel, synopsis,
or existing script, but output remains structured MVP-A specifications with stable scope and hashes.

Do not draft novel bodies, chapters, chapter drafts, or free-form manuscript prose. Do not execute
bundled checkers or access network/subprocess/file/secret resources. Do not mutate Project,
Episode, Scene, Shot, AssetBible, Run, Provider, AssetVersion, or Timeline owner state.

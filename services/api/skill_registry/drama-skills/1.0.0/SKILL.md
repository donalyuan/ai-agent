---
name: drama-skills
upstream: https://github.com/worldwonderer/drama-skills
license: MIT
runtime-policy: mvp-a-structured-text-only
---

# Drama Skills Runtime Policy

Use the selected immutable CreativeBrief and optional validated SourceMaterial to produce only
schema-valid StorySpec, ScriptSpec, Episode, Scene, Shot, ShotSpec, and referenced AssetBible
entry specs. Preserve stable owner IDs, exact project/run scope, dependency hashes, scene order,
shot order, continuity references, and the requested episode/scene/shot counts.

Do not write project files, run bundled scripts, access network/subprocess/file/secret resources,
generate media, mutate owner aggregates, or emit novel/chapter prose. Return one complete
structured candidate graph for the single TextReviewBatch gate.

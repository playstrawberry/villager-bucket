# Bucket Appearance

`villager_bucket.png` is the original first-party flat texture supplied with the
custom-models example. No third-party asset or new artwork is included here.
It is a placeholder appearance, not a finished three-dimensional model.

Villager Bucket registers this file through `custom-models-api`. The resulting
model identity is `villager-bucket.villager_bucket`; the base is `minecraft:paper`
and the maximum stack is one. Names use `bucket_name` from the existing en/fr/es/tr
catalogs. The shared model service owns item construction and resource-pack paths.

Replacing the texture with an original model later must retain that one identity,
registration route and name table. This plugin never sends a resource pack itself.

BiKiNi Normal v2 — native full-layer interactive model

Current actor.json references body.png, arm.png, hat.png, board.png, hair.png,
original.png, roots.png, four alpha masks and hat-front-mask.png.
Only current actor.json references are included. Old cutouts stay in local drafts.

Optional local authoring source, prompts, registration and visual reviews:
../../live2d-drafts/BiKiNi_Normal_v2/README.txt (temporary; not tracked by Git).
Building/running the native application needs only these published PNG/JSON files.
Rebuild with scripts/build-bikini-live2d-v2.py; use --publish after review.
The old cutout preview script is archived under local live2d-drafts only.

Shared idle: breathing, gentle body sway, anchored back-hair movement.
Click board to rock it. Click visible hat/raised arm to slightly lift the hat.
Independent actions can overlap. Dragging does not trigger actions.
Custom native Rust layered/mesh model, not Cubism moc3 or a baked GIF.

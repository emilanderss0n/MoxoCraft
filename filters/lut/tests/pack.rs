//! Behaviour tests on this plug-in's own LUTs, run through `moxocraft_sdk::host`, which feeds
//! bands exactly as PhotoCraft does. (The engine's own tests are in `moxocraft-lut-core`.)

moxocraft_lut_core::pack_tests!(moxocraft_lut, "moxocraft.lut", "../../luts/film-luts");

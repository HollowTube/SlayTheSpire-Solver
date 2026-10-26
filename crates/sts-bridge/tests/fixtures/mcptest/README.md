# MCPTest response fixtures

One raw response line per bridge method, `<screen>/<method>.json`.

**Synthesized, not yet recorded.** `main_menu/` and the `combat*/` sets were
written from the response shapes in the MCPTest bridge source
(`test_mod/Code/BridgeHandler.cs` in HollowTube/sts2-modding-mcp:
`{"result": …, "id": N}` envelope, nulls omitted) because no game was
available. The offline tests assert exact values from them, so they stay as
they are.

- `main_menu/`, `combat/`: the envelope the bridge sends today.
- `combat_unwrapped/`: the same results without the envelope (older bridges).
- `combat_double_encoded/`: the envelope serialized again as a JSON string.

The three combat sets must parse to the same `GameState`.

`recorded/<screen>/` holds real recordings from a live game
(`STS2_RECORD=1`, see the crate README). `recorded_fixtures_parse` checks every
recording that is present. Commit them once recorded, and fix any contract field
they disagree with.

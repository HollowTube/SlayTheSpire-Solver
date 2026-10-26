# MCPTest response fixtures

One raw response line per bridge method, `<screen>/<method>.json`.

**Synthesized, not yet recorded.** These were written from the response shapes
in the MCPTest bridge source (`test_mod/Code/BridgeHandler.cs` in
HollowTube/sts2-modding-mcp: `{"result": …, "id": N}` envelope, nulls omitted)
because no game was available. Re-record them from a live game with
`STS2_RECORD=1` (see the crate README) and fix any field the contract gets
wrong.

- `main_menu/`, `combat/`: the envelope the bridge sends today.
- `combat_unwrapped/`: the same results without the envelope (older bridges).
- `combat_double_encoded/`: the envelope serialized again as a JSON string.

The three combat sets must parse to the same `GameState`.

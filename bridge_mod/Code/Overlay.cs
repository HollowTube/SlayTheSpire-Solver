using System;
using System.Linq;
using System.Text.Json.Nodes;
using Godot;
using MegaCrit.Sts2.Core.Logging;

namespace sts_sim_bridge_mod;

/// <summary>
/// A small always-on-top panel showing the latest sts_sim "analyze" response
/// (state_value plus per-action values), refreshed from
/// <see cref="HookPatches"/>'s analyze pushes. Created lazily on first update
/// and anchored to the top-left of the screen.
/// </summary>
public static class Overlay
{
    private const int Layer = 128;

    private static CanvasLayer? _canvasLayer;
    private static Control? _root;
    private static PanelContainer? _panel;
    private static VBoxContainer? _list;

    // A "deck vs. monster, before any cards are drawn" baseline for the
    // current fight: the mean HP lost over many fresh-shuffle MCTS playouts
    // of the player's master deck against this fight's monster, fetched once
    // via a "deck_baseline" request (see HookPatches.PushDeckBaseline). Null
    // while waiting for the response; _deckBaselineNA=true means no request
    // was sent for this fight (multi-monster room or unmapped monster).
    private static double? _deckBaselineHpLost;
    private static bool _deckBaselineNA;

    // Shown while an analyze request is in flight, appended below the
    // last-rendered rows so the panel keeps showing the previous values
    // (rather than going blank) until the fresh response arrives.
    private static Label? _statusLabel;

    /// Shows or hides a "calculating..." indicator below the current rows.
    /// Safe to call from a background thread. Called from
    /// <see cref="HookPatches"/> around each analyze request so the panel
    /// gives feedback while sts_sim is thinking, without discarding the
    /// last-known values.
    public static void SetCalculating(bool calculating)
    {
        Callable.From(() =>
        {
            EnsureCreated();
            if (_list == null)
                return;

            if (calculating)
            {
                if (_statusLabel == null)
                {
                    _statusLabel = new Label { Text = "sts_sim  calculating..." };
                    _statusLabel.AddThemeColorOverride("font_color", ExplorerDimColor);
                    _list.AddChild(_statusLabel);
                }
            }
            else if (_statusLabel != null)
            {
                _list.RemoveChild(_statusLabel);
                _statusLabel.QueueFree();
                _statusLabel = null;
            }
        }).CallDeferred();
    }

    /// Called when a new fight starts (round 1) to clear the previous
    /// fight's deck-vs-monster baseline until <see cref="SetDeckBaseline"/>
    /// or <see cref="SetBaselineNotAvailable"/> reports this fight's status.
    public static void ResetFightBaseline()
    {
        _deckBaselineHpLost = null;
        _deckBaselineNA = false;
    }

    /// Called when no deck_baseline request will be sent for this fight
    /// (multi-monster room or unmapped monster), so the overlay shows "n/a"
    /// instead of "calculating..." indefinitely.
    public static void SetBaselineNotAvailable()
    {
        _deckBaselineNA = true;
        if (_lastRender != null)
        {
            var (stateHpLost, actualHpLostSoFar, currentHp, maxHp, rows) = _lastRender.Value;
            Callable.From(() => Render(stateHpLost, actualHpLostSoFar, currentHp, maxHp, rows)).CallDeferred();
        }
    }

    // Raw STS2 ids of monsters/cards in the current fight that NameMap
    // doesn't recognize (see StateBuilder.FindUnsupported). Non-empty means
    // this fight's numbers are unreliable - an unmapped monster falls back
    // to a generic placeholder, and unmapped cards are silently dropped from
    // the translated piles.
    private static System.Collections.Generic.List<string> _unknownMonsters = new();
    private static System.Collections.Generic.List<string> _unknownCards = new();

    /// Records this analyze push's unsupported monster/card ids (see
    /// StateBuilder.FindUnsupported), shown as warning rows in Render.
    public static void SetWarnings(System.Collections.Generic.List<string> unknownMonsters, System.Collections.Generic.List<string> unknownCards)
    {
        _unknownMonsters = unknownMonsters;
        _unknownCards = unknownCards;
    }

    /// Records this fight's "deck vs. monster, before any cards are drawn"
    /// baseline (the `mean_hp_lost` from a "deck_baseline" response). Safe to
    /// call from a background thread; re-renders immediately with the
    /// now-known baseline if an "analyze" response has already been rendered
    /// for this fight.
    public static void SetDeckBaseline(double meanHpLost)
    {
        _deckBaselineHpLost = meanHpLost;
        if (_lastRender != null)
        {
            var (stateHpLost, actualHpLostSoFar, currentHp, maxHp, rows) = _lastRender.Value;
            Callable.From(() => Render(stateHpLost, actualHpLostSoFar, currentHp, maxHp, rows)).CallDeferred();
        }

    }

    /// Parses an "analyze" response and (re)renders the panel. Safe to call
    /// from a background thread; the actual scene tree update is marshaled
    /// to the main thread. `actualHpLostSoFar` is the HP the player has
    /// already lost this fight (current fight's starting HP minus current
    /// HP), tracked by <see cref="HookPatches"/>. `currentHp`/`maxHp` are the
    /// player's HP right now, used to turn `expected_hp_lost` into an
    /// end-of-fight HP estimate.
    public static void UpdateValues(string? responseJson, double actualHpLostSoFar, double currentHp, double maxHp)
    {
        if (string.IsNullOrEmpty(responseJson))
            return;

        JsonNode? root;
        try
        {
            root = JsonNode.Parse(responseJson);
        }
        catch (Exception ex)
        {
            Log.Warn($"[sts_sim_bridge_mod] Overlay: failed to parse response: {ex.Message}");
            return;
        }

        if (root is not JsonObject obj || obj["error"] != null)
            return;

        var stateHpLost = obj["expected_hp_lost"]?.GetValue<double>() ?? 0.0;
        var values = obj["values"] as JsonObject;
        var legalActions = obj["legal_actions"] as JsonArray;
        var targetValues = obj["target_values"] as JsonObject;
        // One row per legal_actions entry (in hand order, duplicates and
        // all) rather than per unique key in `values` - sts_sim collapses
        // duplicate card names (e.g. two Strikes) into a single values
        // entry, but the player has a distinct card in hand for each.
        // `bestTarget` is the monster index (0-based) to hit for single-target
        // cards in multi-monster fights, null otherwise.
        var rows = (legalActions ?? new JsonArray())
            .Select(a => a!.GetValue<string>())
            .Select(action =>
            {
                string? bestTarget = null;
                if (targetValues?[action] is JsonObject tv)
                {
                    var best = tv
                        .OrderByDescending(kv => kv.Value?.GetValue<double>() ?? double.MinValue)
                        .FirstOrDefault();
                    const string prefix = "SelectTarget:Monster:";
                    if (best.Key?.StartsWith(prefix) == true)
                        bestTarget = best.Key[prefix.Length..];
                }
                return (action, value: values?[action]?.GetValue<double>() ?? 0.0, bestTarget);
            })
            .ToList();

        _lastRender = (stateHpLost, actualHpLostSoFar, currentHp, maxHp, rows);

        // Callable.CallDeferred is Godot's thread-safe way to run code on the
        // main thread next idle frame, regardless of SynchronizationContext
        // setup (which GodotSharp doesn't configure for Task callbacks).
        Callable.From(() => Render(stateHpLost, actualHpLostSoFar, currentHp, maxHp, rows)).CallDeferred();
    }

    // The parameters of the most recently rendered "analyze" response, kept
    // so SetDeckBaseline can re-render with the now-known baseline as soon
    // as it arrives, without waiting for the next analyze push.
    private static (double stateHpLost, double actualHpLostSoFar, double currentHp, double maxHp, System.Collections.Generic.List<(string action, double value, string? bestTarget)> rows)? _lastRender;

    private static void Render(double stateHpLost, double actualHpLostSoFar, double currentHp, double maxHp, System.Collections.Generic.List<(string action, double value, string? bestTarget)> rows)
    {
        EnsureCreated();
        if (_list == null)
            return;

        foreach (var child in _list.GetChildren())
        {
            _list.RemoveChild(child);
            child.QueueFree();
        }
        // The "calculating..." label (if any) was just freed above along
        // with everything else - drop our reference so SetCalculating(false)
        // (called once the request that triggered this render completes)
        // doesn't try to remove an already-freed node.
        _statusLabel = null;

        // Headline: a chess-engine-style "if the fight ended right now given
        // optimal non-clairvoyant play from here" HP estimate, derived from
        // `expected_hp_lost` (further HP loss from the current position).
        var predictedEndHp = Math.Max(currentHp - stateHpLost, 0.0);
        AddLabel($"sts_sim  predicted end: ~{predictedEndHp:F1}/{maxHp:F0} HP", ExplorerHeaderColor);

        // Warn when this fight's numbers can't be trusted: an unmapped
        // monster falls back to a generic placeholder (wrong move pattern),
        // and unmapped cards are silently dropped from the translated hand/
        // draw/discard/exhaust/deck piles.
        if (_unknownMonsters.Count > 0)
            AddLabel($"unknown enemy: {string.Join(", ", _unknownMonsters)}", ExplorerWarningColor);
        if (_unknownCards.Count > 0)
            AddLabel($"unknown card: {string.Join(", ", _unknownCards)}", ExplorerWarningColor);

        // Next two lines: how this fight's actual-loss-so-far +
        // projected-remaining-loss compares to the baseline expectation
        // captured at the start of the fight. Split across two short lines
        // (with explicit "HP" units) rather than one long line, which wraps
        // awkwardly in the panel.
        var projectedTotal = actualHpLostSoFar + stateHpLost;
        AddLabel($"fight: {actualHpLostSoFar:F1} HP lost so far", ExplorerRowColor);
        var baselineText = _deckBaselineHpLost is double baseline
            ? $"{baseline:F1} HP"
            : _deckBaselineNA ? "n/a" : "calculating...";
        AddLabel($"on pace for {projectedTotal:F1} HP (deck baseline {baselineText})", ExplorerRowColor);

        // Per-action rows: show each legal action's value relative to the
        // best legal action, not an absolute HP figure - the per-action
        // `values`/`action_hp_lost` are rollout-poisoned and on a different
        // scale than the headline `expected_hp_lost` (see server.py's module
        // docstring), so an absolute number here would look contradictory
        // next to the headline. The relative ranking is still meaningful.
        var best = rows.Count > 0 ? rows.Max(r => r.value) : 0.0;
        foreach (var (action, value, bestTarget) in rows)
        {
            var label = action.StartsWith("PlayCard:") ? action["PlayCard:".Length..] : action;
            var delta = value - best;
            var targetSuffix = bestTarget != null ? $" (→{bestTarget})" : "";
            AddLabel($"{label}: {delta:+0.00;-0.00;0.00}{targetSuffix}", ExplorerRowColor);
        }

        Log.Warn($"[sts_sim_bridge_mod] Overlay: rendered {rows.Count} rows, panel rect {_panel?.GetGlobalRect()}");
    }

    private static readonly Color ExplorerHeaderColor = new(1f, 0.85f, 0.3f);
    private static readonly Color ExplorerRowColor = Colors.White;
    private static readonly Color ExplorerDimColor = new(0.6f, 0.6f, 0.6f);
    private static readonly Color ExplorerWarningColor = new(1f, 0.4f, 0.4f);

    private static void AddLabel(string text, Color color)
    {
        var label = new Label { Text = text };
        label.AddThemeColorOverride("font_color", color);
        _list!.AddChild(label);
    }

    private static void EnsureCreated()
    {
        if (_canvasLayer != null && GodotObject.IsInstanceValid(_canvasLayer))
            return;

        var sceneTree = Engine.GetMainLoop() as SceneTree;
        if (sceneTree == null)
        {
            Log.Warn("[sts_sim_bridge_mod] Overlay: Engine.GetMainLoop() is not a SceneTree, skipping");
            return;
        }

        _canvasLayer = new CanvasLayer { Name = "StsSimOverlay", Layer = Layer };

        // A full-rect Control wrapper is required: anchors on a Control only
        // resolve relative to a Control parent's rect, not directly off a
        // CanvasLayer. Mirrors explorer_mod's ExplorerUI root pattern.
        _root = new Control { Name = "StsSimOverlayRoot" };
        _root.SetAnchorsAndOffsetsPreset(Control.LayoutPreset.FullRect);
        _root.MouseFilter = Control.MouseFilterEnum.Ignore;
        _canvasLayer.AddChild(_root);

        // Anchor all four corners to the parent's top-left (0,0) and use
        // positive offsets to size the box - this gives a fixed 360x240 rect
        // independent of content minimum size, clear of the End Turn button
        // and hand of cards docked along the bottom edge. 240px tall
        // comfortably fits a header + one row per hand card (up to 10).
        _panel = new PanelContainer { Name = "StsSimOverlayPanel" };
        _panel.SetAnchor(Side.Left, 0);
        _panel.SetAnchor(Side.Top, 0);
        _panel.SetAnchor(Side.Right, 0);
        _panel.SetAnchor(Side.Bottom, 0);
        _panel.OffsetLeft = 20;
        _panel.OffsetTop = 20;
        _panel.OffsetRight = 380;
        _panel.OffsetBottom = 260;
        _panel.MouseFilter = Control.MouseFilterEnum.Ignore;
        var style = new StyleBoxFlat
        {
            BgColor = new Color(0f, 0f, 0f, 0.6f),
            ContentMarginLeft = 8,
            ContentMarginRight = 8,
            ContentMarginTop = 6,
            ContentMarginBottom = 6,
        };
        _panel.AddThemeStyleboxOverride("panel", style);
        _root.AddChild(_panel);

        _list = new VBoxContainer();
        _list.MouseFilter = Control.MouseFilterEnum.Ignore;
        _panel.AddChild(_list);

        sceneTree.Root.AddChild(_canvasLayer);
        Log.Warn($"[sts_sim_bridge_mod] Overlay: created, layer {Layer}, panel rect {_panel.GetGlobalRect()}");
    }
}

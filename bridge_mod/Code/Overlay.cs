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
/// and anchored to the bottom-right of the screen.
/// </summary>
public static class Overlay
{
    private const int Layer = 128;

    private static CanvasLayer? _canvasLayer;
    private static Control? _root;
    private static PanelContainer? _panel;
    private static VBoxContainer? _list;

    /// Parses an "analyze" response and (re)renders the panel. Safe to call
    /// from a background thread; the actual scene tree update is marshaled
    /// to the main thread.
    public static void UpdateValues(string? responseJson)
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

        var stateValue = obj["state_value"]?.GetValue<double>() ?? 0.0;
        var stateHpLost = obj["expected_hp_lost"]?.GetValue<double>() ?? 0.0;
        var values = obj["values"] as JsonObject;
        var hpLost = obj["action_hp_lost"] as JsonObject;
        var legalActions = obj["legal_actions"] as JsonArray;
        // One row per legal_actions entry (in hand order, duplicates and
        // all) rather than per unique key in `values` - sts_sim collapses
        // duplicate card names (e.g. two Strikes) into a single values
        // entry, but the player has a distinct card in hand for each.
        var rows = (legalActions ?? new JsonArray())
            .Select(a => a!.GetValue<string>())
            .Select(action => (
                action,
                value: values?[action]?.GetValue<double>() ?? 0.0,
                hpLost: hpLost?[action]?.GetValue<double>() ?? 0.0))
            .ToList();

        // Callable.CallDeferred is Godot's thread-safe way to run code on the
        // main thread next idle frame, regardless of SynchronizationContext
        // setup (which GodotSharp doesn't configure for Task callbacks).
        Callable.From(() => Render(stateValue, stateHpLost, rows)).CallDeferred();
    }

    private static void Render(double stateValue, double stateHpLost, System.Collections.Generic.List<(string action, double value, double hpLost)> rows)
    {
        EnsureCreated();
        if (_list == null)
            return;

        foreach (var child in _list.GetChildren())
        {
            _list.RemoveChild(child);
            child.QueueFree();
        }

        AddLabel($"sts_sim  state value: {stateValue:F2}  (~{stateHpLost:F1} HP lost)", ExplorerHeaderColor);
        foreach (var (action, value, hpLost) in rows)
        {
            var label = action.StartsWith("PlayCard:") ? action["PlayCard:".Length..] : action;
            AddLabel($"{label}: {value:F2}  (~{hpLost:F1} HP lost)", ExplorerRowColor);
        }

        Log.Warn($"[sts_sim_bridge_mod] Overlay: rendered {rows.Count} rows, panel rect {_panel?.GetGlobalRect()}");
    }

    private static readonly Color ExplorerHeaderColor = new(1f, 0.85f, 0.3f);
    private static readonly Color ExplorerRowColor = Colors.White;

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

        // Anchor all four corners to the parent's bottom-right (1,1) and use
        // negative offsets to size the box - this gives a fixed 320x220 rect
        // independent of content minimum size. Bottom offset is raised to
        // -120 (rather than flush with the corner) to clear the End Turn
        // button and hand of cards docked along the bottom edge. 220px tall
        // comfortably fits a header + one row per hand card (up to 10).
        _panel = new PanelContainer { Name = "StsSimOverlayPanel" };
        _panel.SetAnchor(Side.Left, 1);
        _panel.SetAnchor(Side.Top, 1);
        _panel.SetAnchor(Side.Right, 1);
        _panel.SetAnchor(Side.Bottom, 1);
        _panel.OffsetLeft = -340;
        _panel.OffsetTop = -340;
        _panel.OffsetRight = -20;
        _panel.OffsetBottom = -120;
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

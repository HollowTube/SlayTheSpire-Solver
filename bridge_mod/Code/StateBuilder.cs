using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.MonsterMoves.Intents;

namespace sts_sim_bridge_mod;

/// <summary>
/// Translates a live STS2 CombatState into the JSON wire format
/// python/sts_sim/server.py's "analyze" command expects.
/// </summary>
public static class StateBuilder
{
    private const int DefaultIterations = 200;

    /// Names of cards/monsters dropped because NameMap doesn't recognize
    /// them, so each is logged at most once per combat.
    private static readonly HashSet<string> _loggedUnsupported = new();

    public static string BuildAnalyzeRequest(CombatState combatState, Player player)
    {
        var root = new JsonObject
        {
            ["cmd"] = "analyze",
            ["iterations"] = DefaultIterations,
            ["seed"] = (long)(ulong)Random.Shared.NextInt64(),
            ["state"] = BuildState(combatState, player),
        };
        return root.ToJsonString();
    }

    private static JsonObject BuildState(CombatState combatState, Player player)
    {
        var pcs = player.PlayerCombatState;
        var creature = player.Creature;

        var state = new JsonObject
        {
            ["player"] = BuildPlayer(creature, pcs),
            ["hand"] = CardNames(pcs?.Hand),
            ["draw_pile"] = CardNames(pcs?.DrawPile),
            ["discard_pile"] = CardNames(pcs?.DiscardPile),
            ["exhaust_pile"] = CardNames(pcs?.ExhaustPile),
            ["turn"] = combatState.RoundNumber,
            ["monsters"] = BuildMonsters(combatState),
        };
        return state;
    }

    private static JsonObject BuildPlayer(Creature creature, PlayerCombatState? pcs)
    {
        return new JsonObject
        {
            ["hp"] = creature.CurrentHp,
            ["max_hp"] = creature.MaxHp,
            ["energy"] = pcs?.Energy ?? 0,
            ["max_energy"] = pcs?.MaxEnergy ?? 0,
            ["block"] = creature.Block,
            ["statuses"] = Statuses(creature),
        };
    }

    private static JsonArray CardNames(CardPile? pile)
    {
        var names = new JsonArray();
        if (pile == null)
            return names;

        foreach (var card in pile.Cards)
        {
            var entry = card.Id.Entry;
            if (NameMap.CardNameMap.TryGetValue(entry, out var name))
            {
                names.Add(name);
            }
            else if (_loggedUnsupported.Add($"card:{entry}"))
            {
                MegaCrit.Sts2.Core.Logging.Log.Warn(
                    $"[sts_sim_bridge_mod] Dropping unsupported card from translated state: {entry}");
            }
        }
        return names;
    }

    private static JsonArray Statuses(Creature creature)
    {
        var statuses = new JsonArray();
        foreach (var power in creature.Powers)
        {
            if (NameMap.PowerNameMap.TryGetValue(power.Id.Entry, out var name))
            {
                statuses.Add(new JsonArray { name, power.Amount });
            }
        }
        return statuses;
    }

    private static JsonArray BuildMonsters(CombatState combatState)
    {
        var targets = combatState.Players.Select(p => p.Creature).ToList();
        var monsters = new JsonArray();
        foreach (var creature in combatState.Enemies)
        {
            monsters.Add(BuildMonster(creature, targets));
        }
        return monsters;
    }

    private static JsonObject BuildMonster(Creature creature, List<Creature> targets)
    {
        var entry = creature.ModelId.Entry;
        var mapped = NameMap.MonsterNameMap.TryGetValue(entry, out var mappedName);
        if (!mapped && _loggedUnsupported.Add($"monster:{entry}"))
        {
            MegaCrit.Sts2.Core.Logging.Log.Warn(
                $"[sts_sim_bridge_mod] Unmapped monster {entry}, sending generic placeholder");
        }

        var (intentId, attackDamage) = NextMoveIntent(creature, targets);

        var monster = new JsonObject
        {
            ["name"] = mapped ? mappedName : null,
            ["hp"] = creature.CurrentHp,
            ["max_hp"] = creature.MaxHp,
            ["block"] = creature.Block,
            ["statuses"] = Statuses(creature),
            ["attack"] = attackDamage,
        };
        if (mapped && intentId != null)
        {
            monster["intent"] = intentId;
        }
        return monster;
    }

    /// Returns (move id, total attack damage if the intent is an attack else 0).
    private static (string? intentId, int attackDamage) NextMoveIntent(Creature creature, List<Creature> targets)
    {
        var move = creature.Monster?.NextMove;
        if (move == null)
            return (null, 0);

        var attackDamage = 0;
        foreach (var intent in move.Intents)
        {
            if (intent is AttackIntent atk && intent.IntentType == IntentType.Attack)
            {
                try
                {
                    attackDamage += (int)atk.GetTotalDamage(targets, creature);
                }
                catch
                {
                    // best-effort: leave attackDamage as accumulated so far
                }
            }
        }
        return (move.Id, attackDamage);
    }
}

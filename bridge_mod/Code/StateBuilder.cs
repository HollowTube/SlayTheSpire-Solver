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
    // 200 iterations of pure-random-rollout MCTS makes state_value swing by
    // ~0.06 between calls on an otherwise-unchanged state - distractingly
    // "noisy" in the overlay. 1000 cuts that spread to ~0.02 for ~20ms per
    // request, still negligible for an async background push.
    private const int DefaultIterations = 1000;


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

    /// Builds a "deck_baseline" request for the player's master deck against
    /// the current fight's monsters. Unknown STS2 monster IDs are resolved
    /// by Rust via MonsterId::from_sts2(); unrecognised monsters use the
    /// generic placeholder (attack from intent, repeated each turn).
    public static string? BuildDeckBaselineRequest(CombatState combatState, Player player)
    {
        var enemies = combatState.Enemies.ToList();
        var targets = combatState.Players.Select(p => p.Creature).ToList();

        var monstersArray = new JsonArray();
        foreach (var creature in enemies)
        {
            var (intentId, _) = NextMoveIntent(creature, targets);
            var monsterObj = new JsonObject { ["name"] = creature.ModelId.Entry };
            if (intentId != null)
                monsterObj["intent"] = intentId;
            var statuses = Statuses(creature);
            if (statuses.Count > 0)
                monsterObj["statuses"] = statuses;
            monstersArray.Add(monsterObj);
        }

        var root = new JsonObject
        {
            ["cmd"] = "deck_baseline",
            ["deck"] = CardNames(player.Deck),
            ["monsters"] = monstersArray,
        };
        return root.ToJsonString();
    }

    /// Returns the raw STS2 ids of any unsupported entities in the current fight.
    /// Currently always returns empty lists — unknown cards and monsters are
    /// handled gracefully by the Rust sim (silently dropped or generic placeholder).
    public static (List<string> unknownMonsters, List<string> unknownCards) FindUnsupported(CombatState combatState, Player player)
    {
        var unknownMonsters = new List<string>();
        var unknownCards = new List<string>();

        return (unknownMonsters, unknownCards);
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
            names.Add(card.Id.Entry);
        return names;
    }

    private static JsonArray Statuses(Creature creature)
    {
        var statuses = new JsonArray();
        foreach (var power in creature.Powers)
            statuses.Add(new JsonArray { power.Id.Entry, power.Amount });
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
        var (intentId, attackDamage) = NextMoveIntent(creature, targets);

        var monster = new JsonObject
        {
            ["name"] = creature.ModelId.Entry,
            ["hp"] = creature.CurrentHp,
            ["max_hp"] = creature.MaxHp,
            ["block"] = creature.Block,
            ["statuses"] = Statuses(creature),
            ["attack"] = attackDamage,
        };
        if (intentId != null)
            monster["intent"] = intentId;
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

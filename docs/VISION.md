# Vision — what TheCity is for

Written 2026-10-05 from Dylan's words. The milestone specs (`SPEC.md`, `M8_*` … `M14`) are the contract for what gets built next; this file is the contract for *why*, so that any system choice can be checked against it.

## The end state

A full simulation of a cyberpunk city that a player can **go break, take over, or simply exist in**. A place to make a lot of little stories. The sim is the product; the player is one more agent with a human brain, added after the sim stands on its own (see `thecity-player-character-later`).

The city must run on an average PC. Realism is bought with design, not CPU: everything new is daily, hourly or event-driven, and off-screen lives are decided fast, attributed lazily and narrated on demand (`M10_SCALE.md`).

## The stories it has to be able to tell

- **Friends.** You make friends. Then your friend is killed by a gang member and you go off to avenge them.
- **Word gets around.** When you kill someone, people hear. Individuals come hunting you for revenge, and hunt each other the same way. Revenge chains are a first-class loop, not a special case.
- **Assassinations are popular.** Hired guns and fixers are ordinary businesses. A hit is a contract with a price, a buyer, a target and a risk, and anyone with the money can place one, including corps, gangs, and the player.
- **Quests are everywhere, and they are real.** A quest is a goal an NPC or an organisation actually holds and would otherwise hire an NPC to fulfil. The player takes the job the sim was about to give to someone else. No scripted quest exists; the quest board is the city's open contracts.
- **Rich lives on inspection.** Any agent, watched or not, has a biography that makes sense against the rest of the sim. An off-screen murder has a murderer when you need one, and the answer never changes.

## What that implies for systems (beyond M14)

| Need | System |
| --- | --- |
| Word gets around | Gossip: memories propagate second-hand along edges with decay; reputation per agent and per faction derived from what is known, not from what happened. |
| Revenge | Grudges as edges with a target and a cause that survive the memory cap; a Hunt goal that plans across days; vendettas that chain (the avenger becomes a target). |
| Hired guns, fixers, assassinations | Contracts as a first-class entity: buyer, target, price, deadline, status; a Fixer business that matches contracts to agents with the skills; the law treating a fulfilled hit as a Murder with the buyer as accessory when the word gets around. |
| Quests | The contract entity again, surfaced to the player: corp orders (`Secure`, `Acquire`, `Lobby`), gang orders (`Raid`, `BreakOut`), and personal goals (`Court`, `Bury`, revenge) all become takeable jobs with the same payout the NPC would have got. |
| Break, take over, exist | The player can found a gang or a business through the same entry points NPCs use (`Register`, the `JoinGang` bootstrap), own assets, hire, and be hunted. |

## How we know it works

The sim is numerical. Whether it is working is decided from data first: the CSV report, the event log, the scenario gates and the parity tests. A story that cannot be seen in the numbers is not yet in the sim. Watching it live in the app is the second check, not the first.

## The world outside the city (added 2026-10-05, Dylan)

There is an economy beyond the city. **Megacorps exist as an economic idea outside it**: the whole outside world runs at a very low LOD, a ledger of resources and intents, never bodies. A megacorp's presence in the city (its buildings, its exec, its guards) is a branch, not the whole. That has two consequences:

- **Destroying a megacorp completely is hard.** Wipe out its city holdings and the parent can send agents in from abroad to take territory back, fund a new branch, or buy up what is left. Driving it out is a campaign, not a raid.
- **There are still ways to kill one.** Cyber attacks (Virt, M14: Data theft, ICE, decks) and economic warfare (undercutting, strikes, monopolies broken, supply cut off) hit the ledger the parent runs on. A megacorp dies when its outside resources run dry, not when its last city building falls.

The same is probably true of every faction: gangs can have brothers in the next city, the law has a state behind it. Since the game will ultimately be first person, all of this is abstracted. It enters a player's game only through its effects: groups having more or fewer resources, more or fewer bodies, a branch that is or is not reinforced. Nothing outside the city is ever simulated per agent.

Design implications for the milestones: the M11 corp needs room for a parent (an outside treasury and a reinforcement rule), M12 districts give the parent territory to retake, M14 Virt gives the player and rival corps the only weapon that reaches the ledger directly. A later milestone (after M14) gives the outside world its own daily tick.

## Dialogue, leverage and the social stats (added 2026-10-05, Dylan)

At the closest level of detail there is a **dialogue system**. A player close enough overhears conversations between NPCs, and NPCs in range converse with the player character. The conversational possibilities should be deep: language models can generate complex dialogue trees with many options for handling a situation with an NPC, making friends, allies, enemies and romances. No dialogue is scripted ahead; the tree is generated from the sim state (who this is, what they know, what they want, what they fear, what they hold against you).

**Story LOD follows the player.** An NPC who interacts with the player regularly has their story promoted to a higher LOD so it stays robust: more of their life is decided per agent rather than per table, and the player gets regular updates from them (phone calls, emails, messages). The pinned-Full rule for the player extends to the player's circle.

**A psychological simulation with a few levels of detail and stats.** Persuasion, Intimidation, Knowledge and the like as agent stats; **relative strength** and **reputation** weigh every social move. You are more likely to be intimidated by a giant with a gun and the best implants. A name known as a big business owner opens doors a broke nobody cannot: a broke guy cannot get a meeting with a megacorp CEO, the biggest arms dealer in town might. Reputation is what is known about you (gossip, above), not what you did.

**Leverage is a first-class tool, for everyone.** Taking someone hostage gives leverage over someone else for information or access to a building. Extortion, blackmail, threats, fraud: all available to the player character and to NPCs and factions alike, through the same mechanisms. The sim already has extortion as a gang action; it generalises to a social move with a target, a demand, a threat and a credibility derived from relative strength and reputation.

Design implications: social stats and reputation belong to the sim before the player exists (NPCs use them on each other; the gossip and revenge systems read reputation); leverage moves are the contract entity again (buyer, target, price, deadline) with a threat instead of a payment; the dialogue layer is a renderer over the social state and never a second source of truth; the stat and reputation model lands with gossip (post-M14), the generated-dialogue layer with the player character.

## Governance, missions and the outside as combat maps (added 2026-10-05, Dylan)

**Governance.** Governments and corporations have a governance model. Some are dictatorships (one leader, one Personality driving the brain, as the gang leader does today); some have shared leadership, a **board**. Most of the time a board is abstracted as a corporate personality (the aggregate of its members' Personalities drives the brain). But at first-person detail the members are agents, and a player could bribe, blackmail or threaten every member of a board to get themselves voted in as the leader of a megacorp. Super hard, not impossible for a determined player with enough power, especially one who can send agents on missions.

**Missions at three LODs.** A player can send agents on missions (a raid, a hit, a heist, a board member's persuasion). Each mission can be:

- **watched in real time** through cyberspace, if the soldiers are equipped with the right technology (Virt, M14: the deck is the camera);
- **resolved off screen** and rendered as a probability (the M8 brawl resolver and the M10 binder already do this for NPC actions);
- **carried out by the player character** in person.

The same contract (M16) backs all three; only the renderer differs.

**The outside world as limited combat maps.** Missions outside the city (an asset in the outside ledger, M17: a rival's depot, a parent's data centre) are small maps with a limited cast of NPCs who need no persistence beyond the moment. The exception is the point: if the player befriends one, or makes a specific enemy of one, they can become persistent and later show up in the city seeking out the player character, for good or ill. The outside world's agents enter the city the way everything outside does, as a resource effect, until a story promotes one of them to a person.

Design implications: a `Governance` on every faction (Dictator(agent) | Board(members, vote rule)) whose brain reads an aggregate Personality; leadership change as a vote the leverage moves can swing; missions as contracts with a renderer flag; a "promoted from outside" immigrant with a grudge or a friendship edge to the player as the hook from a combat map back into the city.

## Economy, tech, skills and verticality (added 2026-10-05, Dylan)

**A robust, fluid economy.** Supply and demand across many goods, not one. Different types of food and food production, with the dystopian twist: legitimate operations like vertical farming and fish farms beside a Soylent-style factory whose raw material is people abducted off the street, and scavengers who abduct anyone whose implants look expensive to harvest the chrome, for profit and to upgrade themselves. Every one of these is a business an NPC corp or the player can run.

**Tech trees and tech decay.** A tech tree gates what can be built and used. Technology can be lost over time if research is not kept up (Songs of Syx's rule). Data can be stolen and deleted (M14), so a company can lose its most advanced technologies when its research cannot sustain them or its Data is taken.

**Competence is people.** A corporation's or government's effectiveness is an aggregate of its management's stats and its employees' stats for the relevant actions, so good employees matter. Characters with high stats in specific skills are rare. You can render a company or a government ineffective by murdering its smartest managers and scientists; the skill loss cripples it. Employees can be poached from other groups with better pay, or with threats and extortion.

**Hostages and private prisons.** Megacorps routinely keep prisons filled with hostages for leverage over people. Freeing people from private prisons is a mission type.

**Reputation everywhere.** The player character has a reputation with each faction, and factions have reputations with each other.

**Verticality.** The city needs layers of verticality to feel like a true metropolis.

Design implications: goods as a typed resource with per-good supply, demand and price (M13 adds the second good; the generalisation follows); production chains with inputs, including the illegal ones whose input is a person (abduction as a leverage move, M16, feeding a factory); skills as the Full-tier stat model with a rarity distribution, and `Corp` effectiveness terms read from the skills of exec and staff; a research upkeep rule in the M14 tech tree with decay and Data loss; a `Prison` building kind for corps with hostages as contracts; reputation as a matrix of faction × faction plus player × faction, derived from what is known (gossip); map layers (a `z` on `TilePos`, portals between layers, the Virt plane as the first second layer) as a later map milestone.

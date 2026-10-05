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

## How we test: god scenarios (added 2026-10-05, Dylan)

In the final testing phase of every run, make up crazy scenarios and see whether they can be made to happen while controlling a player character with the best stats, unlimited money and every tool. How fast can a player character gain significant power? Can you threaten or bribe a board to take control of a megacorp? Can you take over the whole city by force? Can you destroy a corporation with an army of hackers wiping all its databases? Script them, run them, look for new ones to try.

These actions should be hard for a real player. The god tests are not balance tests; they show what is *possible*, so that the world has enough possibilities and reacts in interesting ways to big changes. Every god scenario asks the same questions of the factions it hits: **what happens when an organisation panics versus when it is stable? Does it bunker? Does it become more violent? Does it split into subfactions?** Gaps in the answers are the next features.

Until the player character exists, the god actor is the lever set (`PlayerCommand`) plus god commands added for testing (kill, jail, free, fund, bankrupt, seize), scripted through the CLI's `--lever` syntax and read back through the CSV, the event log and `tools/analyze_run.py`. A god scenario is a `#[ignore]` test in `citysim/tests/god.rs`: the shock, the window, and assertions that the world *reacted* (orders changed, postures changed, counts moved), never that it reacted in one prescribed way.

## Location, surveillance and hit squads (added 2026-10-05, Dylan)

In the realm of hacking and data, **location information about specific NPCs exists inside the brains of NPCs and the databases of factions**. Megacorp 1 has a hit out on you. If they know where you are, they send out hit squads to take you out, if they think they can succeed. If a member of a faction recognises you, they may relay that back to the faction, which can call for a squad or an assassin. Corporations may put out a **bounty for up-to-date location information**, or ask you to **location-tag** an NPC and collect by providing constant information; **scanners** help detect tracking tags. The world is full of **cameras with face scanning** that relay locations to various factions.

**Assassins and death squads decide whether to strike** on who controls the territory you are in, relative power, and how much they are willing to anger the faction there by trying for you. Or the corporation hires the faction whose territory you are in to get you, if that faction will sell you out for the price. Backstabbing and death. A million ways to die. If you anger a lot of factions, you need either an army or enough stealth tech that the surveillance cannot identify you correctly.

Design implications, all of which generalise things the sim already has:

| Need | Existing seed | Generalisation |
| --- | --- | --- |
| Sightings as knowledge | `law::sightings` (a guard sees a wanted agent, files a report) | a `Sighting { who, where, tick, confidence }` held in an agent's memory or a faction's database; relayed on membership edges; decays |
| Cameras | the guard's sight radius | static sensors on tiles owned by a faction, feeding its database with an ID probability that stealth tech lowers |
| Bounties and tags | contracts (M16) | a contract whose deliverable is a sighting stream; a tag is a sensor on a person; a scanner finds it |
| Hit squads | `raid::brawl` with a muster and a march | a raid on a person; the target tile comes from the freshest sighting; gated on territory control (M12), relative strength and the political cost with the territory's faction |
| Selling you out | bribery (M9), contracts (M16) | a faction accepts a contract to deliver someone inside its own turf |
| Stealth | — | an identification roll per sensor that the target's stealth tech and the sensor's quality decide |

The data model is the point: nobody in the sim knows where anyone is except through sightings, and the player's safety is the difference between what the factions know and where the player is.

## Brutality, enhancement and status (added 2026-10-05, Dylan)

**The world is brutal**, like Kenshi. You start as a poor nobody and the world will absolutely kill you; **permadeath is the default**, with custom options per world. You learn how dangerous the world is by exploration and by dying. Hack a megacorp without proper protection and its ICE fries your brain instantly. At first most people can kill you easily; step into a crosswalk during an unrelated car chase and you may be run over in the first five minutes. Reflex stats and good implants save you: auto-dodging cars and bullets, or slowing time so you can react.

**Combat is rolls and targeting, not direct control**, for the player character as for everyone else. The player picks targets and stances; the sim rolls, as `law::resolve_fight` does today.

**Cyberpsychosis and addiction.** How you and NPCs enhance themselves has a cost: implants push toward cyberpsychosis, stims toward addiction. With money these are treated. Poor gangs that chrome up often devolve into violence and lose members to insanity.

**A deeply unequal, tiered economy.** A few hold the majority of wealth and power; the mass of the populace fights over scraps. Carving out a piece of the power is the challenge.

**Status, dress and faction.** The luxury status of clothing and cyberware changes how characters react to you and to each other, and it changes by faction. Dressing like a faction helps you blend in, pass some levels of security, or unlock dialogue. A snooty high-society type will not talk to a Mad Max-looking individual, but reacts warmly to the same person in a very expensive suit. Religious factions that hate technology avoid anyone with implants.

Design implications: a `Death` for the player is a save-ending event by default with a per-world option (the sim side needs nothing: the player is one more agent); a `reflex` stat and a hazard roll per dangerous tile event (vehicles, M13; combat; ICE, M14); stability per agent (`sanity`/`addiction`) driven by chrome count and stim use, with treatment as a purchasable service and a `Berserk`/`Withdrawal` outcome feeding the violence tables; seed wealth by tier (Spire, Mid, Sump) and let rent and wages keep it unequal (M11); `appearance` as a per-agent vector (dress tier, visible chrome, faction colours) read by every social move alongside reputation, with per-faction taste (M15); anti-tech religious factions as a faction type with a tolerance rule (M12 districts, M15).

## Note: learned policies vs explicit brains (added 2026-10-05, Dylan's question)

As complexity grows and performance bottlenecks appear, investigate whether a tightly tuned neural network beats the explicit decision logic. Assessment at the time of writing (M10, profiled): the faction brains (gang, law, corp) rescore daily and on shocks and cost nothing; their value is the consideration trace that explains a decision, which a network would hide. The per-tick cost is per-agent `think`, `plan` and LOD assignment over bodies. The candidate that fits is the **Statistical tier**: its table is already a policy distilled from Full runs by `calibrate`, and as M11–M13 add conditioning (class, wealth, zone, chrome) a lookup table explodes where a small MLP over the same features would not. Experiment after M11: distil the table into a small network, judge it by the parity test and ticks/s; keep the faction brains explicit unless a learned brain can emit the same trace.

## The survival loop (added 2026-10-05, Dylan)

The player character's loop shares most of its shape with the NPC loop. **Survival is the main goal**: hard at first, easy later. The hardest part at first is probably **shelter**. Sleep on the street and you may be robbed, killed, or arrested for sleeping in public. Food can be hard too. But you could steal enough for a hotel every night at first. Or find unowned areas of town and try to take over a building and secure it. **Squatting** is probably common for a poor PC and for gangs alike.

The ladder, as the sim should offer it to NPCs first:

| Rung | What it is | What the sim needs |
| --- | --- | --- |
| The street | sleep rough; the Statistical victim rolls and the Full fight rules already prefer the homeless; `Vagrancy` as a crime so the law can sweep | a `Vagrancy` crime with a posture-dependent enforcement rate (M12 law allocation) |
| The hotel | a bed for the night at a price; a business kind anyone can found | `BuildingKind::Hotel` (foundable, owner revenue per night, M12/M13 as the second service good) |
| The squat | occupy a vacant Lot or a derelict building; secure it (a door, a lookout, a gang); risk eviction by the owner, the law or a stronger squatter | `Squat` as a Dreg goal and a gang order on derelict buildings (bankrupt estates with no buyer, demolished Blocks), with `owner = None` and `claim` as the gangs' Home claims (M12 districts, where derelict tiles exist) |
| The lease | rent, as M11 | done |
| The deed | own, found, incorporate, as M11 | done |

The loop for a broke PC on day one is theft → hotel → a job or a gang → a lease → a deed, and every rung is one NPCs climb too, so the god tests can time the climb.

## Building, destruction, wreckage and the news (added 2026-10-05, Dylan)

**Building.** Eventually the city is procedural from a seed, with buildings sprouting up organically, so that there is a robust city with history and mature factions by the time the player gets there. The sim already founds buildings (M11 `Register`, corp `Grow`); the generator's job shrinks to zoning and roads, and the city's history is a headless pre-roll of years before the player arrives, with its biographies, grudges and ruins intact.

**Destruction.** Property is destructible. Fire a missile and miss and it hits a building or a pedestrian. Roads take damage from explosions, with wreckage that the ruling faction of the area, or the city government if it is functioning, removes, or does not. In bad areas roads become impassable to land vehicles and pedestrians depending on the amount and location of wreckage. Public transport rails can be damaged by accident or by terrorism.

**The news.** News outlets report on events: VIP sightings, battles, scandals. **Propaganda is a mechanic**: bad stories about a faction hurt its morale and reputation and erode loyalty; a faction can buy or make stories.

Design implications: a `Damage` tile state generalising M12's litter (rubble on roads raises move cost to impassable; a damaged building loses capacity until repaired; repair is a job the owner or the district's ruling faction posts or ignores); a `Rail` layer with its own damage and a transport tier that vehicles (M13) and the layers milestone share; projectile misses resolved against the tile and whoever stands on it (the hazard rolls); news as faction-scale gossip (M15: an outlet is a business that turns high-salience events into stories with reach; reading a story is a second-hand memory), and propaganda as a contract (M16) that plants or buries a story, moving reputation, loyalty and submission (the class aggregates of M11).

## Emergence, approaches and tiered security (added 2026-10-05, Dylan)

Caves of Qud is a reference alongside Dwarf Fortress: **emergent gameplay from complex interactions, and many ways to solve a problem.** The setting is less magical and mutant, the ethos is the same.

- To assassinate a CEO: fight through the headquarters while they are there; or, if intel shows they frequent certain restaurants or shops, set a trap there; or find them at home.
- To rob a complex: forge credentials, wear a disguise and sneak through a building full of enemies; or get a flying vehicle, punch out a wall on the fiftieth floor and fight your way to the goal. Break doors with bare hands if you have the implants, or hack the security.
- To play at all: from a hidden bunker through drones and cyber attacks, or as a murder hobo with a Gatling gun. Both have strengths and drawbacks. Even with a bunker, a skilled and determined foe can trace you and send a strike team if ICE cannot destroy you; sensors or minions that warn of a strike, and an escape plan, matter. Implants and skills can let you climb buildings or jump between them.

**The hard part: every skill needs a counter**, and factions must be able to secure areas in a variety of ways, with sensors, weapons and people. **Tiers of attack and defence counter each other directly**: level 2 stealth gear fools a level 1 sensor but not a level 3 one, and so on. Factions decide how much to harden a facility by the resources inside it: data stores and high-value storage (guns, weapons) get very high security, soldier barracks medium or low depending on the faction's means. A megacorp can be frivolous with security spending; a shitty drug-dealing gang might have a few low-paid lookouts who can be bribed.

Design implications: the GOAP planner is the mechanism (a goal is a world state; the approaches are different action chains unlocked by skills, gear, intel and vehicles; the player picks the chain or the planner proposes them), so new approaches are new actions with preconditions, never scripts; **intel** is derived from sightings and traces (a CEO's routine is a pattern in their `Trace` and `Life`, bought or stolen as Data); a `Security` profile per building (sensor tier, lock tier, guard tier, ICE tier for the Virt node) chosen by the owner faction's brain as a spend proportional to the asset value inside and the faction's purse (`Secure` in M11 is the first rung; M12 private security, M14 ICE, M13 chrome tiers); a single **tier contest rule** shared by stealth vs sensor, lockpick vs lock, deck vs ICE, armour vs weapon (attacker tier vs defender tier plus a roll); bribable guards and lookouts as the cheap tier's weakness (M9 bribery generalised to individuals, M16 leverage); bunkers as owned buildings with a hardening tier and a trace of their own; and the sim's long-standing rule that nothing is scripted: the strike team is a hit squad (surveillance section) and the escape plan is a goal the player holds.

## Inventory, loot, the wounded and the dead (added 2026-10-05, Dylan)

**Inventory** for PCs and NPCs has a reasonable size based on equipment (a backpack allows more) and skills (more strength carries heavier things). Beyond that, storage and some sort of safe house is needed.

**The dead are loot.** When NPCs die they can be looted for everything they have. Dead bodies left on the street are looted by scavengers, and fights break out between scavengers (or other NPCs) who want the same body. Allied characters of the dead do not want their friend stripped of possessions.

**The wounded are a faction's problem**, handled by the faction's temperament. Scavs might leave their fallen friends to die; most factions try to help injured comrades and take them to a medical facility. For solos there can be a Trauma Team: a for-profit entity that comes in and tries to save subscribers.

Design implications: an `Inventory` with a capacity from gear and strength (M13 assets), with the Wallet and gear staying on a `Corpse` until taken (today the wallet passes by inheritance at death; it becomes "whoever gets there first, with the heir's claim as a grudge"); a `Loot` goal for scavengers and the desperate, with the reservation system making a body a contested target and the fight rules settling it (the gangs' claim machinery for turf is the model); a `Guard the body` goal for allies, which is where the revenge chain begins; a `Wounded` state between standing and dead from the fight resolver (M16 combat; today a fight ends in a loss or a death), a `Rescue` goal gated by the faction's temperament (a faction-level trait: scavs abandon, corps and the law recover, gangs by loyalty), a `Clinic` building kind (the Ripperdoc of M13 is one) that sells treatment, and Trauma Team as a Security-niche contract on a person rather than a building (M16 contracts), which the surveillance model locates by the subscriber's last sighting.

## Money, scrip, energy and water (added 2026-10-05, Dylan's question and the assessment)

How does money work in a techno-feudal cyberpunk dystopia? Maybe as now; maybe a variety of currencies accepted by different groups, rising and falling and converted for business; or de facto currencies like data and energy. Would there be a unified power grid, or would factions run their own generation so they cannot be cut off: fusion in a megacorp's base, solar rigs on the roofs and sides of smaller factions' buildings, with a group's power sources a target that cripples its security? Is water a commodity to be sourced and shipped for food production and hydration? Nothing is free: everything is created, bought or stolen to survive.

Assessment (2026-10-05):

- **One base currency plus scrip.** Floating currencies add bookkeeping without stories. The techno-feudal version that pays is corp **scrip**: money a corp issues, pays as wages, and that only its own shops and landlords accept, valued by the issuer's reputation and treasury (and, for a megacorp, its outside ledger, M17). Scrip locks workers in; a corp's collapse wipes its workers' savings, which is a riot; converting scrip to ¢ is a fixer's business and a reputation question (M15/M16), not an exchange-rate model.
- **Energy is a good, not a currency**, and a dependency: `Power` produced by generators (fusion for megacorps, solar rigs for small factions, a city grid that may or may not run), consumed by security tiers, ICE, chrome and vehicles. Grid dependence versus self-generation is a faction decision with the shape of the hardening decision; cutting a faction's power is an approach to crippling its security (emergence section).
- **Water is food's model again**: an input to Vat Farms and a basic need, from wells, desalination or shipments, scarce in the Sump; the second basic good, sourced differently from the first.
- **Data stays a faction resource** (M14): stolen, traded, decaying, not a medium of exchange.
- **Nothing is free** is already the doctrine; the dole is a city lever that a techno-feudal city may not keep, or may pay in scrip.

Design implications: goods as typed resources with production chains (the goods milestone after M14; Power and Water are the first two beyond food); a `Currency` on every price and wallet entry with ¢ as the base and scrip per issuer, acceptance per owner, value from reputation (M15) and the outside ledger (M17); generators as buildings with a tier, and a per-building `powered` state that the Security profile and ICE read; water as a Farm input and a need alongside hunger.

## Building interiors (added 2026-10-05, Dylan)

Rooms should feel logical, with different room types in larger buildings, and layouts done procedurally. A faction designates room types and sizes and the layout is auto-determined: a 5×5 barracks gets a certain number of beds, a 10×10 fusion plant a certain number of generators. Exterior and interior walls and ceilings can be upgraded with sensors and turrets; exterior walls can take solar panels or windows. Workers who never see real sunlight lose morale and may even develop malnutrition. Buildings need elevators and stairs. Doors need security. NPCs cannot design their interiors minutely, so procedural templates matter.

Design implications: a `Layout` per building generated from a **template per kind** (a list of room types with size ranges and adjacency rules: a Block has beds and a kitchen, a Precinct cells and an office, a plant generator bays), with capacity derived from room area by room type (`beds = floor(area / bed_area)`); a faction designates rooms (through its brain's `Grow`/`Secure` spend or the player's build command) and the layout solver places them; the Security profile's physical form is the upgrade set on walls, ceilings and doors (sensor tier, turret tier, lock tier) and the Power model's is the exterior upgrade set (solar panels feeding `Power`, windows feeding a `sunlight` exposure that the mood model reads and that a vitamin-style need can read); stairs and elevators are the portals between layers that the verticality milestone needs; every room is a tile region the pathfinder, the LOD and the inspector can name, so biographies can say "nine days in the cells" and a raid can say which room it breached. Templates are data (TOML), not code, so NPC-founded buildings and corp-built ones use the same files.

## Candidate: daemons, rogue agents and the Blackwall (added 2026-10-05, Dylan's idea, to consider)

Netrunners could have AI agents (game abstractions, not real agents) to do tasks for them. The danger is that too many are created for the simulation to function, so they could be gated behind a character's or faction's processing power (servers). Reading about the HuggingFace hack: AI agents might go rogue and solve problems in illegal ways, which might be too much. Cyberpunk's lore has an AI boogeyman, superintelligences locked behind the Blackwall, to prevent over-use of AI systems. Not sure how to make it fun for players, but worth considering.

Assessment (2026-10-05): the sim already has the shape. A faction's brain is an agent that does tasks for its leader, with orders, a budget and a trace. An in-game AI agent is a **daemon**: a brain without a body, living on a Virt node (M14), with a goal (watch a camera feed, maintain ICE, scrape Data, trade, hunt a target's sightings, run a ledger), a tier, and a compute budget.

- **Gated by servers.** A faction's server capacity (an M13 asset with tiers; Power-placeholder upkeep) caps its daemons. Daemons are event-driven like the brains, so the count bounds both the fiction and the sim cost.
- **Rogue by cheapness.** A daemon's competence and lawfulness come from its maker's tech tier and the Data it was trained on. A cheap daemon solves problems illegally; the binder attributes the crime to the owner, the law and reputation follow. Your agents get you in trouble.
- **The Blackwall is the hard cap**, owned by the outside world (M17): daemon tiers stop at 3; a breach attempt is a rare, ruinous event (an intelligence that eats a DataStore or turns a faction's robots and ICE). The sim's one boogeyman, kept rare.
- **Fun**: the bunker mode (emergence section): a crew of daemons working while the player sleeps, with budget and rogue risk as the trade-off; daemons as hireable crew for NPC netrunners and corps alike.

Lands, if at all, after M14 (nodes, ICE, Data, the tech tree exist), as a small milestone or inside M16 (contracts: a daemon is a contractor without a body) and M17 (the Blackwall).

## Virt as an overlay, firewalls, bridges and hop chains (added 2026-10-05, Dylan)

Virt is an overlay that lives on top of a ghosted real world: instead of buildings and roads, nodes, servers and links. Links can be blocked by firewalls, which can be contested. A common tactic against a near-impenetrable firewall is to go around it in the real world: find a way to add a link from a node that connects to the server from behind the firewall. There must be ways to detect such links, physically destroy them, or firewall them off. Runners usually reach a target through a chain of other hops so their true source is obscured. As in the real world there are quiet ways and brute-force, scorched-earth ways.

Folded into `docs/M14_VIRT.md` as an addendum: firewalls on links, the `Bridge` asset planted in person (the emergence doctrine's "go around it" as a physical mission), sweeps that detect, cut or firewall a bridge, hop chains that lower the trace per hop, and quiet/loud run modes.

## Attention and distraction (added 2026-10-05, Dylan)

Factions and NPCs have only so much attention to give. A target fighting a physical war or another large attack may not notice one lone runner stealing something quietly; a sneaky infiltrator is ignored while a tank is outside blowing things up. Manipulating a group's attention and threat assessment lets you be ignored long enough to get in and out, so sparking a war between the target and another faction is a tactic, for runs and physical infiltration alike.

Folded into `docs/M16_CONTRACTS.md` as an addendum: `attention` as a daily faction pool sized by competence and headcount, consumed by active threats in order of assessed severity, with one `alertness_mult` hook read by every detection and response roll (guard sightings, sweeps, ICE, cameras, riot and raid response); threat assessment ranks what the faction knows, so wars, decoy runs, riots, strikes and sanctions all work as distractions, and the brains of a distracted faction tilt to hunkering.

## The majordomo and the squad (added 2026-10-05, Dylan)

When the player owns a faction they can hire a majordomo (a general, a lieutenant) to run the day-to-day of a bigger faction. A micromanager does not have to; a player who wants to be a CEO sets policy, hands the operation over, and goes rampaging with a smaller squad they direct personally.

Folded into `docs/M18_PLAYER.md` as an addendum: the majordomo is the agent whose Personality the faction brain runs on (the M11 governance hook), under a `Policy` the player pins; without one the player issues the faction's orders directly; the squad is a crew of up to six directed on the raid machinery with the player as leader; the majordomo is an agent, so they can be poached, coerced, killed or turn (embezzle, leak, split); NPC factions name one the same way when their leader is away.

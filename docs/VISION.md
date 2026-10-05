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

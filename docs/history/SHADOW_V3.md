# Shadow pass V3: jobs and lives

2026-10-16, seed 42, the Jobs & lives phase-8 tree (main 4a9df24 plus the gate's work: the scavs' fix, the organ market order). The milestone's new archetypes get picks and **diaries only, no bounds** (`docs/JOBS_AND_LIVES.md` § 12; bounds after the eight-run ensemble). Breadth over depth: this feeds the NPC-shadowing session that comes next. Every event is a seeded game abstraction.

```
citysim-cli shadow --seed 42 --start-day 19 --days 7 --count 2 --pick joytoy,vendor,cabbie,courier,office_worker,arms_dealer,forger,elder,loader,builder,foreman,madam,companion,cultist,teacher,scav,dancer --out shadow_v3/d19
citysim-cli shadow --seed 42 --start-day 90 --days 7 --count 2 --pick street_worker,pimp,day_labourer,black_doc,arms_dealer,scav,cultist,joytoy --out shadow_v3/d90
```

38 diaries (30 on day 19, 8 on day 90); regenerable with the commands above. The new picks (`citysim-cli/src/shadow.rs`): a trade's post or pitch by its `[[trades]]` key (joytoy is the chip trade's Host, cabbie the Cab Driver), the pimp from the vice state, the foreman of each Yard, the arms dealer (a gang dealer whose gang keeps weapons), the elder (`frail_from_years` and over), the cultist (a member of a gang under a cult row), the scav (a member of a gang typed with organ targets), the teacher. **No candidate** on either day: pimp, day labourer, black doc; street worker only on day 90.

The critiques, as lives: `docs/shadow_v3/street_and_the_old.md` (vendor, street worker, elder, scav), `docs/shadow_v3/work.md` (builder, foreman, loader, courier, cabbie, office worker, teacher), `docs/shadow_v3/vice.md` (joytoy, companion, madam, dancer), `docs/shadow_v3/gray_and_faith.md` (forger, arms dealer, cultist; the missing black doc and pimp).

**Verdict.** The most alive diaries are the gang lives the milestone added: the **scav** (a killer who stays poor, and a city that answers with 15 grudges), the **loader** turned cult abductor, the **forger** (sales, a bribed guard, an arrest, her papers exposed), the **cultist** (tithes, extortion, apostates sworn against), the **arms dealer** who is a corp clerk by day. The trades read as working lives but thin: the **builder**, **courier**, **teacher**, **office worker** and **joytoy** work their shifts and rest; the **vendor** is a business when the spot is near home and a commute when it is not. Not alive: the **cabbie** (no fare driven on screen), the **madam** and **dancer** (paid shifts and empty days). Not there: the **pimp**, the **day labourer** and the **black doc** (no candidate on seed 42).

## Gaps, ranked by how many archetypes each distorts

`[bug]` a mechanism doing the wrong thing; `[feature]` a mechanism that does not exist yet.

1. **Idle hours after the shift** (madam, dancer, companion, office worker, joytoy, forger; 40-60 h a week of Rest) `[feature]`. V2's cause 8 still stands for the new trades: the evening has no menu beyond a venue and a hang-out.
2. **The work is a counter** (cabbie, loader, teacher, joytoy, madam) `[feature]`. ClerkWork with no customer: the cab is never driven, no crate is lifted, no child is taught, no client is named. V2's cause 5 for the new trades.
3. **Distance** (vendor 153 min to the spot, courier 60-72 h a week walking, joytoys 35-54 h of travel) `[feature]`. The pitch spot and the hire ignore the home.
4. **The seeded ages vs the trades** (builder 72, office workers 64-67, a 74-year-old scav winning an abduction, a 19-year-old foreman) `[feature]`. Ages to 75 and nothing retires a worker or weighs age in a fight; tenure starts at day 0 for everyone.
5. **The rare trades are absent** (pimp, day labourer, black doc, street worker on day 19) `[feature]`. The street pitches are few; no black clinic is built; a day post lives inside a day.
6. **A killer stays poor** (scav) `[feature]`. The take goes to the gang's stock and the organs rot; the member's cut is nothing.
7. **The cult that harvests chrome forbids it** (scav, loader, cultist) `[bug]`. The Drowned Choir is typed as scavs and is the cult row; it expels the member it paid to chrome.
8. **The cost of the chip is invisible** (joytoy) `[feature]`. The diary shows a shift with a premium; the debt that signed it and the memory it takes do not show.
9. **Faith without worship** (cultist) `[feature]`. Tithes and hunts, no Preach in a member's week.
10. **Elders without kin** (elder) `[feature]`. A childless elder scavenges; nobody takes her in.
11. **The robbed do not arm** (`person.rs::probe_robbed_resident_buys_a_weapon_within_3_days`, ignored) `[bug]`. The robbery forms the want; the Shop offer stays on chrome.

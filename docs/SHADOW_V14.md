# Shadow V14: pressure pass round 5 at 100k (2026-10-28)

Round 5's read, in V13's window so the two compare one for one: main `f5893b7` (`target/shadow_v14/sha.txt`), the
100k island, seed 42, days 21-27, two picks per archetype plus round 5's new picks (`neglect_parent`,
`camp_jobless`); no pimp stood on day 21, so the one pimp in the city was shadowed from day 22
(`target/shadow_v14/isle100k_s42_d22_pimp/`). Seven Sonnet critics (`docs/plans/shadow_v14.md`) wrote
`docs/shadow_v14/<group>.md`. Runs in `target/shadow_v14/`; the round's 30-day 100k counters in
`target/r5_100k/analyze.txt` (round 4's in `target/r4_100k/`).

## Verdict

**Better than V13 where round 5 aimed, flat elsewhere; one phase took a trade away.** What round 5 fixed shows at
100k:
- the forger forges: eight forged weapons licences in five days, then reported and arrested (`forger_8215`, U5); the
  fixer's book is the liveliest thing in crime: posted, taken with a crew, failed with a reason, the cut (U5);
- a ripperdoc and an orderly work a real 09:00-17:00 clinic day: the doctor's cut, the blood wash, chrome booked by
  the hour (care, U6);
- the neglect take says why ("unfed 3 days again, inside the warning of d18"), the `[family]` line reads the Camp
  right after a take, the Camp's headcount reconciles (street, U2);
- the starving reach a soup pot and a dry pot is told (street, U3); 100k starvation deaths 30 -> 7;
- guards have a night (patrol, arrest, escort; corp), the bouncer turns people away in corp's diaries;
- the jobless arc runs end to end inside one diary (search, hire, a move nearer work, a theft, a sentence; labour);
- U8 left gang choices reading as V13's (GangWork goals per archetype in the same range, the same plan shape).

**The under-18 rule held in every diary**: every harm line prints ages, no critic found a minor as a victim, in
vice, hit or dead (contract lines print no target age, so a contract's mark cannot be checked from the diary). Two
toddlers sat on the street for days with nothing fetching them: no harm, but it reads broken.

What reads wrong: the neglect ladder's warnings hang at 100k (three groups), U6's trip bound left one pimp in the
city, violence with no law after it, empty Schools and clinics next to full ones, gang life without a raid.

## The 30-day counters (100k seed 42 x 30, round 4 -> round 5; FINDINGs, nothing tuned)

| | r4 | r5 |
|---|---|---|
| starvation deaths | 30 | 7 |
| Starving events | 13,404 | 6,403 |
| school days / child-days | 6.5 % | 18.0 % (114 Schools; founded on demand 0) |
| Witness | 268,796 | 162,347 |
| ChildTaken / ch_camp daily mean / ch_street | 677 / 91 / 266 | 624 / 119 / 249 |
| deaths by violence / Murder | 431 / 227 | 441 / 236 |
| thefts / arrests' jailed mean | 18.7k / 1,141 | 17.5k / 1,052 |
| homeless max / Materialised | 13.1k / 597 | 13.3k / 612 |
| pimps daily mean / street visits | 1.63 / 229 | **0.2 / 150** |
| fake papers sold / licences sold | 14 / 120 | 12 / 158 |
| chrome installs | 4,858 | 3,947 |
| ticks/s mean (shared with the shadow run: a print) | 25 | 24 |

## Round 6's input, ranked by what the player sees first

1. **The neglect ladder hangs at 100k** (`OPEN-231`; vice, street, gangs, crime): warnings standing since d5, d7,
   d14 with empty pantries and no second warning or take (`camp_parent_102044`, `scav_46032`, `owner_broke_6426`
   still "on the warning" after his jailing on d26, `jailed_71789`'s children evicted onto the street d26 untaken);
   one warning names 18 children of ten families at one door (`owner_broke_6426 d21 00:10`); two toddlers out of the
   Camp on the street for days (`camp_jobless_61284` d24-27, `miserable_12664` d21-23; `OPEN-242`).
2. **U6's trip bound emptied the street** (`OPEN-217`): `pimp_trip_ticks` 120 names almost nobody at 100k (the
   street's members live a district away: `pimp_40493 d23 00:07` "living 197 tiles from the pitches"); collections
   43 -> 3; the named pimp also hired as a Vat Tech (`d22 00:11`).
3. **Violence with no law after it** (`OPEN-238`): a murder and a strip with no Report, corpse found or arrest
   (`courier_48760 d25 05:25`); the wounded on Flee 72 h at energy 0.01 with the ExecWage paying (`ceo_91004`,
   `guard_8429`); a member dying mid-Extort still collects (`gang_member_6572 d22 14:24`); ten strangers empty one
   till in six hours, no report (`vendor_9648 d23`); a Fixer "closed by the law" keeps working (`fixer_40483 d26`).
4. **Empty rooms beside full ones**: teachers with 1-2 pupils (the fee; `teacher_15873`), Sump and corp-district
   children with no School within 1 h (`OPEN-232`); two of four clinic posts with no patient and a ripperdoc struck
   off unexplained (`ripperdoc_11838 d26`), one back room with 574 patients and the other none (`OPEN-217`).
5. **The night out refused at the door** (`OPEN-239`): the Unwind plan drinks before it gambles, the bouncer turns
   the drunk away, nightly in three corp diaries (`office_worker_56309 d23 21:23`), the "barred till" ignored.
6. **Homes and fares far from work** (`OPEN-240`): a move-in with family 475 tiles from the post, 503 min each way
   (`night_shift_27693 d23 00:08`); a 7c cab leg on a 7c wage paid to a different stranger each time
   (`fabber_84414`); a paid cab that lands in the wrong district (`exec_73953 d24 01:16`).
7. **Gang life without a raid** (`OPEN-241`): 0 musters and 0 raids in 16 gang diaries; one door paying three gangs
   in 15 hours (`Block#478`); the leader's two voices (V13's); `BountyPaid` printed three or four times a sighting.
8. Still labels: the door that turns nobody away and the fighter with no bout (vice, `OPEN-217`), the gun and the
   buyer who never fire or buy, no arms_dealer (crime), the office desk (corp).

# God scenarios v10: the El

2026-10-11. One scenario for [docs/TRANSIT.md](TRANSIT.md) (§ 7), in `citysim/tests/god.rs`. It asserts that the world reacted to the shock, not a calibration band.

## `god_line_cut`

**Setup.** Seed 42 on the shipped config with `[transit] enabled` (on by default since phase 3). The Vat Line (line 0) is cut by god `CutLine { line: 0, days: 10 }` at day 20 of a 45-day run. Riders on the line are sampled hourly on days 10-20 (anyone with a filed ride on line 0).

**Asserted.**
- The line carried somebody before the cut.
- No ride on the line during the cut.
- The Vat Line riders' mean `life::commute_cost` (Home door to work door, the walk or the ride where it wins), for those still housed and employed two days into the cut, is longer than before it.
- The city's fares fall during the cut.
- Rides on the line resume after it reopens.

**Measured (2026-10-11, phase 3).**
- 117 Vat Line riders on days 10-20, 72 of them housed and employed.
- Their commute cost averaged 60 ticks before the cut and 98 during it; 19 of the 72 got longer.
- Rides on the line: 0 during the cut, 629 after it.
- Fares: 5,846 on days 10-20, 1,832 on days 20-30, 6,389 on days 30-45.

**Reading.**
- The Vat Line is the network's spine. The Low Line joins it at (231, 127), so a cut also strands the deep Sump's trips to the Vats, and fares fell by two thirds.
- 45 of the 117 riders had no Home and job to measure (errands, the jobless, the homeless): the El is not only a commute.
- Of the 72 commuters, 19 got a longer structural commute (the mean rose by two thirds, those 19 carrying it). The rest have another line, or their commute already walked.

**Gaps.**
- A cut makes nobody move house (relocation reads the shared distance only for the far, and a 10-day cut is shorter than the housing demand's hold).
- Nothing tells the story of the cut: no event for a stranded rider, no grudge against the city.

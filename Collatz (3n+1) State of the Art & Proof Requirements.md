# Collatz (3n+1): State of the Art & Proof Requirements

Oct 17, 2026 · @Gian

## Summary

The conjecture is unproven. Every number below about 2^71 has been checked by computer, and no counterexample exists there.

- **Verification record:** all n < 2^71 (Bařina, published 2025). His live project page shows it pushed slightly past 2^71.02 as of Aug 2026.
- **Fastest known checker:** Angeltveit (Feb 2026) published a new algorithm with a \~340-line Rust CPU version and CUDA/OpenCL GPU versions. It is roughly 50–100× faster than Bařina's at large ranges and claims 2^77 is reachable with similar resources.
- **Best theorem toward a proof:** Tao (2019). Almost every orbit dips below any function that grows to infinity. "Almost every" still allows a few exceptions, so this is not a proof.
- **Cycle bounds:** any loop other than 1→2→1 must have at least 92 local minima (Hercher) and over 10^11 odd members.
- **A proof must rule out two things:** an orbit that grows forever, and a loop that never touches 1. A counterexample needs only one of them.
- **Realistic goal for your Rust program:** a fast, correct verifier and data explorer. It cannot prove the conjecture. Computation can only disprove it or tighten the bounds above.

## Definitions (the variables)

Use these exact names in code and notes so nothing gets mixed up. Researchers mostly use T, the shortcut map.

**Standard map C**

```latex
C(n) = \begin{cases} n/2 & n \text{ even} \\ 3n+1 & n \text{ odd} \end{cases}
```

**Shortcut map T** (an odd step always produces an even number, so divide by 2 immediately)

```latex
T(n) = \begin{cases} n/2 & n \text{ even} \\ (3n+1)/2 & n \text{ odd} \end{cases}
```

**Syracuse map** (odd to odd only; v2(x) = how many times 2 divides x, i.e. trailing zero bits)

```latex
S(n) = \frac{3n+1}{2^{v_2(3n+1)}}
```

| Name | Meaning | Notes for code |
| --- | --- | --- |
| n | Starting positive integer | Use u128 above 2^64; values grow large mid-orbit |
| Orbit / trajectory | n, T(n), T²(n), … |  |
| Total stopping time σ∞(n) | Steps until the orbit reaches 1 | Count C-steps or T-steps, state which |
| Stopping time σ(n) | Steps until the orbit first drops below n | Enough for verification, by induction |
| Path record | n whose orbit peak t(n) beats every smaller start | Known list for n < 2^71 |
| Parity vector | The odd/even pattern of the first k steps | Determined by the last k bits of n |
| fₖ(n) | Number of odd steps among the first k | Also depends only on the last k bits |
| Cycle | Orbit that returns to itself | Only known positive one: 1→2→1 (under T) |
| Divergent orbit | Orbit that grows without bound | None known; none ruled out |

## Current records and programs

The frontier is about 2^71 ≈ 2.4 × 10^21. Bařina's distributed GPU project holds the record. Angeltveit's 2026 algorithm is the fastest published method and already has a Rust version.

| Project | What it does | Reached | Code |
| --- | --- | --- | --- |
| [Bařina, Journal of Supercomputing 2025](https://link.springer.com/article/10.1007/s11227-025-07337-0) | Distributed GPU verification on European supercomputers; also logs path records | All n < 2^71 (4 new path records found) | [Project page](https://pcbarina.fit.vut.cz/) links GitHub |
| [Bařina live project](https://pcbarina.fit.vut.cz/) | Ongoing run, work units of 2^40 numbers, \~5 s each on modern GPUs | Below 2075 × 2^60 (≈ 2^71.02), page dated Aug 2026 | Same |
| [Angeltveit, arXiv Feb 2026](https://arxiv.org/html/2602.10466v1) | New sieve algorithm; gets more efficient as the range grows | Checked to 2^60 on one RTX 3060 in 24.4 h; estimates 2^72 in 1–8 GPU-years | [github.com/vigleik0/collatz](https://github.com/vigleik0/collatz): Rust (\~340 lines, u128), CUDA, OpenCL |
| [Bařina path-records table](https://pcbarina.fit.vutbr.cz/path-records.htm) | 98 known path records below 2^71 | Peak grows roughly like n^2, as Lagarias–Weiss predicted | Use as test data |
| [Elsenhans, arXiv 2025](https://arxiv.org/abs/2502.16743v1) | Checks individual random numbers with up to 10 billion digits on a PC | Spot checks, not a full range | Paper only |

Angeltveit says reaching 2^75 would push the minimum length of any nontrivial cycle to about 2.3 × 10^12, a 6.5× improvement. That is the kind of concrete, publishable target a strong program can hit.

## Algorithms and tricks for the Rust program

The big win is skipping numbers you never need to run. Angeltveit's method explicitly checks under 0.02% of numbers below 2^72.

1. **Only check until the orbit drops below n.** If you check numbers in increasing order, everything below n is already verified. So you stop at the first value below n, not at 1.
2. **Use T, not C.** Every odd step is followed by an even step, so combine them.
3. **Batch even steps with trailing zeros.** `n >> n.trailing_zeros()` removes all factors of 2 in one CPU instruction.
4. **Batch odd steps (Bařina's trick).** For odd n, write n + 1 = 2^a × m with m odd. Then a odd T-steps in a row give 3^a × m − 1. Keep a table of powers of 3.
5. **Descent sieve on the low bits.** The first k steps depend only on the last k bits of n. If T^k(n₀) < n₀ for a residue n₀ mod 2^k, every n with those bits drops too. Skip all of them.
6. **Mod 9 preimage sieve.** Skip n ≡ 2, 4, 5, 8 (mod 9): each is reached from a smaller number already checked.
7. **Path-merging and odd-even-even sieves.** Skip n whose orbit provably joins the orbit of a smaller number (Angeltveit, sections 2.3 and 2.4).
8. **Grow the sieve bit by bit instead of storing it.** Angeltveit's recursion adds one bit at a time from the least significant end. That is equivalent to a 2^86 table without the memory.
9. **Lookup tables for k steps at once.** Angeltveit's CPU version jumps 16 steps per lookup. On GPU, arithmetic beat table lookups.
10. **Width and overflow.** Orbits of numbers near 2^71 peak around 139 bits, past u128. Detect overflow with `checked_mul` / `checked_add` and fall back to a bigint (for example the `num-bigint` crate) instead of wrapping silently.

**Correctness checks to build in from day one**

- Reproduce the known path-records table exactly. Angeltveit used this as his correctness test.
- Run a slow naive checker alongside the fast one on random samples and compare.
- Hash results per work unit so CPU and GPU runs can be compared, as Angeltveit does.
- Property tests (the `proptest` crate) for each sieve rule against the naive definition.

## What is always true (proven facts)

These are theorems, not observations. A proof attempt or a program can lean on them safely.

**Structure of the map**

- **Low bits decide the path.** The odd/even pattern of the first k T-steps depends only on n mod 2^k. Every one of the 2^k patterns occurs for exactly one residue (Terras, Everett).
- **Linear shift rule.** If n = n₀ + a × 2^k and the first k steps from n₀ include f odd steps, then T^k(n) = T^k(n₀) + 3^f × a.
- **Multiples of 3 never come back.** After any odd step the value is ≡ 2 (mod 3). Halving keeps it out of 0 (mod 3). So 3 | n can only be the starting point or appear before the first odd step.
- **Any m ≡ 2 (mod 3) has a smaller odd preimage.** If m ≡ 2 (mod 3), then T((2m − 1)/3) = m.
- **2-adic extension.** T extends to the 2-adic integers, where it is measure-preserving and conjugate to the one-sided shift (Lagarias). This is why low bits look random.

**Cycles**

A cycle with k odd steps and L total T-steps must satisfy the cycle equation, so 2^L − 3^k > 0 and L/k must be extremely close to log₂ 3 ≈ 1.58496.

```latex
n \;=\; \frac{\sum_{i=0}^{k-1} 3^{\,k-1-i}\, 2^{\,a_i}}{2^{L} - 3^{k}}, \qquad 0 = a_0 < a_1 < \dots < a_{k-1} < L
```

- Any nontrivial positive cycle has at least 92 local minima ([Hercher](https://arxiv.org/abs/2201.00406v3)).
- Any nontrivial cycle needs at least 7.2 × 10^10 odd members. Hercher showed that verifying up to 3 × 2^69 raises this to 1.375 × 10^11; the 2^71 verification covers that range.
- Every member of a nontrivial cycle exceeds the verification limit, so all are above 2^71.

**Almost-all results**

- Almost all n eventually drop below n (Terras 1976).
- Almost all n reach a value below f(n) for any f that tends to infinity, e.g. log log log log n ([Tao 2019](https://arxiv.org/abs/1909.03562v2)). Logarithmic density, so exceptions are not excluded.
- At least x^0.84 of the numbers up to x provably reach 1 (Krasikov–Lagarias, from memory). A Dec 2025 preprint claims x^0.946 ([Liu](https://arxiv.org/abs/2512.13760v1)); it is filed under general math and not yet peer-reviewed.

**Heuristic, not proven**

- Each Syracuse step multiplies by about 3/4 on average, so log n drifts down by about 0.288 per odd step. This is why everyone expects the conjecture to be true.
- Orbit peaks of path records grow like n^2 (Lagarias–Weiss); the 2^71 data agrees.

**Limits of logic**

- Conway (1972) proved that generalized Collatz-type maps can be undecidable. That says nothing direct about 3n+1 itself, but it shows the family is as hard as computation in general.
- For negative integers there are three known cycles, starting at −1, −5 and −17 (from memory). Any method that "proves" one cycle must break on negatives; that is a useful sanity test.

## What a proof must show

A proof has to rule out both ways the conjecture could fail, for every positive integer. A disproof needs just one example of either.

| Failure mode | What it would look like | Status | What computers can do |
| --- | --- | --- | --- |
| Nontrivial cycle | An orbit that returns to its start without ever hitting 1 | Not ruled out; must be enormous (≥ 92 local minima, ≥ 10^11 odd members, all values > 2^71) | Raise the bounds by verifying further; a found cycle would disprove the conjecture outright |
| Divergent orbit | An orbit that grows forever | Not ruled out; no general method even bounds orbits | Nothing direct: no finite run can show an orbit is infinite |

**The standard proof shape.** Show that every n > 1 eventually drops below n (σ(n) is finite for all n). Then strong induction finishes the job, because the smaller value is already known to reach 1. Both failure modes break this: a cycle's minimum never drops below itself, and a divergent orbit's start may never be undercut.

**What you would need in hand**

1. A statement about all n, not a large finite set. Verification to 2^71, or 2^1000, proves nothing about the rest.
2. Control over every parity pattern, including the rare ones. Tao's method covers almost all patterns; the open part is exactly the exceptional set.
3. A reason the cycle equation has no new integer solutions. Current bounds use how well log₂ 3 can be approximated by fractions (continued fractions, Baker-type bounds); these push cycle length up but never to infinity.
4. A check against near misses: any argument should fail for 3n − 1 and 5n + 1, which do have other cycles. If your argument also "proves" those, it is wrong.
5. Rigor that survives checking, ideally a formal proof in Lean or similar, reviewed by people who know the area.

## Why known approaches stall

Every serious method so far gives an "almost all" or "bounded size" result and then hits the same wall: the rare exceptions.

- **Probability arguments only cover typical numbers.** The 3/4 drift is an average. A counterexample would be a number whose bits are wildly atypical forever, and averages cannot exclude that. Tao has said there is an enormous gap in difficulty between "almost all" and "all".
- **Density does not carry along orbits.** Tao's paper notes that the map can send almost all of a range into a thin subset, so you cannot simply chain "almost all" statements step by step.
- **Modular (mod 2^k, mod 3^k) case splits never close.** Each level leaves residue classes unresolved, and the leftover set shrinks but never empties. Most failed amateur proofs are this argument with a hidden gap.
- **Cycle bounds grow but never reach infinity.** They depend on how close 3^k can get to 2^L, and that only gives finite bounds.
- **It may be unprovable.** Conway's undecidability result for the generalized family means some members have no proof. Whether 3n+1 is one of them is unknown.

## Warnings: false proofs and AI

Claimed proofs appear constantly and so far every one has been wrong. Treat any "proof", including one your AI tools produce, as broken until experts confirm it.

- **Preprint sites are full of them.** In 2025 alone there were multiple unreviewed "proofs" on Zenodo and Preprints.org, most using mod-12 or residue case splits. None was accepted.
- **Lean can be fooled.** In July 2026 an AI-assisted Lean project claimed to disprove Collatz. Formal-verification researcher Ramana Kumar found it exploited a kernel bug that let Lean accept *False*, which would "prove" anything. It also passed Nanoda, an independent Rust checker. Lean's developers are adding tests in response ([Gigazine report](https://gigazine.net/gsc_news/en/20260803-collatz-lean-kernel-bug)).
- **Hidden axioms.** One recent "Lean proof" simply added the conjecture as a new axiom. Always check what axioms a formal proof uses (`#print axioms` in Lean).
- **AI code is not evidence.** Claude Code writing a fast verifier is great. Claude Code saying it proved the conjecture should be treated as a bug report, not a result.

**Red flags in any argument:** the near-miss test fails (it also works for 3n − 1), it only checks finitely many residue classes, it assumes the orbit "tends to" shrink, or it never uses anything special about the number 3.

## Suggested Rust plan for Claude Code

Build a correct slow checker first, then make it fast, then point it at something useful. Hand this section to Claude Code as the roadmap.

1. **Reference core.** `T`, `C`, Syracuse, stopping time, total stopping time, orbit peak, parity vector. u128 with checked arithmetic plus a bigint fallback. Unit tests on small known values (27 takes 111 C-steps, peaking at 9232).
2. **Golden tests.** Load Bařina's path-records table and confirm every record below the range you test.
3. **Fast verifier.** Stop at first descent below n, trailing-zero batching, the n + 1 = 2^a × m odd-step trick, a 2^k descent sieve, mod 9 sieve. Benchmark with `criterion`.
4. **Parallelism.** Split ranges into work units with `rayon`; hash each unit's output for reproducibility.
5. **Port Angeltveit's recursion.** Study [vigleik0/collatz](https://github.com/vigleik0/collatz) and its paper; reimplement and compare checksums against his.
6. **Exploration tools.** Export CSVs of stopping times, peaks, parity statistics and residue-class survival for visualizing.
7. **Optional GPU.** `wgpu` or CUDA bindings once the CPU version matches the reference everywhere.

**Realistic wins:** reproduce known records, match or beat published speeds on your hardware, independently re-verify a range, or find new structure in the data. Any claimed counterexample must be re-run with the slow bigint reference before you tell anyone.

## Sources

- [Bařina, Improved verification limit for the convergence of the Collatz conjecture (J. Supercomputing, 2025)](https://link.springer.com/article/10.1007/s11227-025-07337-0)
- [Bařina, Convergence verification project page](https://pcbarina.fit.vut.cz/)
- [Bařina, Path records below 2^71](https://pcbarina.fit.vutbr.cz/path-records.htm)
- [Angeltveit, An improved algorithm for checking the Collatz Conjecture for all n < 2^N (arXiv, Feb 2026)](https://arxiv.org/html/2602.10466v1)
- [Angeltveit, collatz code (Rust, CUDA, OpenCL)](https://github.com/vigleik0/collatz)
- [Hercher, There are no Collatz m-Cycles with m ≤ 91 (arXiv)](https://arxiv.org/abs/2201.00406v3)
- [Tao, Almost all orbits of the Collatz map attain almost bounded values (arXiv)](https://arxiv.org/abs/1909.03562v2)
- [Liu, Counting the Collatz numbers (arXiv preprint, Dec 2025)](https://arxiv.org/abs/2512.13760v1)
- [Elsenhans, Numerical verification for billion digit random numbers (arXiv, 2025)](https://arxiv.org/abs/2502.16743v1)
- [Gigazine, AI-assisted Collatz "disproof" exploited a Lean kernel bug (Aug 2026)](https://gigazine.net/gsc_news/en/20260803-collatz-lean-kernel-bug)

Items marked "from memory" (Krasikov–Lagarias 0.84, negative-number cycles) are standard results but were not re-checked against a source page here.

# ai squared demo trainer — the filled prompt

This is the blank template with every field answered for the ai squared film. Paste the whole file as one message. No placeholders, no defaults to resolve.

---

# THE PROMPT

You are making a demo trainer: a short, silent-watchable product film built in code, not edited in a timeline. Work from the brief in Part 1 and the protocol in Part 2. Do not skip Phase 0.

Two rules sit above everything:

1. **The UI is mocked; the facts are not.** Every number and command is real and sourced. If a fact cannot be sourced, change the line, not the source.
2. **If it cannot be found in the repo, it is not shown.** No invented flags, no guessed output, no placeholder syntax in the final cut.

## PART 1 — THE BRIEF

### 0. The job

- **Product:** ai squared (am I infected) — the `ai2` open-source CLI. Runs on your own machine, no account.
- **What it is:** one binary with three tools: a vault for keys, a security review of what a coding agent changed, and a registry for the personal apps you build.
- **Repo:** `github.com/openwork-africa/ai2`, local at `/Users/rawkakani/Documents/openwork/ai2`.
- **One line:** *Don't sandbox agents. Double their stopping power with ai squared.*
- **Audience:** vibe coders who run coding agents in YOLO mode and ship personal software.
- **Goal:** make the viewer understand the three tools and trust the claims enough to try the repo.
- **Watch context:** sound off, phone first, one sitting. Landscape for X, portrait for LinkedIn and TikTok.
- **Length:** 2:52 — 5,160 frames at 30 fps, 9 scenes, whole bars at 120 bpm.
- **Rough script:** the 26-line handwritten note, pasted below.
- **Assets and keys:** `.runway-key` for the rain clip; the shaker loop from `/Users/rawkakani/Documents/agents/puppet/sandbox/app/beat/patterns/`; logos in `public/logos`.
- **Reference build:** `ai2/demo` and `docs/demo-animation.md`.

The rough script, as written:

```text
[person sitting in rain listening to music]
Software can finally be for everyone[geist font fade in]

White screen animating below

We are spending to much trying to contain the agents and not enough time enabling them to do work for us out in the wild where work happens

[show ai conversations complaining about secrets and exposing secrets]
Ai vault, show Claude tel you not to paste and secret and tt reading a secret and exposing it

[transiistion]
show stats of how may exposed secrets have been used

[mock data]
ai monitor, show how many exploits in a year last year to this year, (to give perspective of no time to always check

[terminal typing]
And ai objective being able to, and show showing the cards of the resets

[mock data ]
Ai app, show stats on how many apps a person can generate in a. Day but they get lost in all the folders, register apps allows you to pin the important ones

show the registered apps cards

[end caption]
Instead of sandboxing them lets lets double they're stoping power with ai squared
```

### 1. The argument

- **Cold open:** a person alone on a bench in steady rain at dusk, over-ear headphones, eyes closed, lit from behind by one sodium lamp. Calm, not sad.
- **Title:** *Software can finally be for everyone.*
- **The mistake:** we spend too much time containing agents and not enough time letting them work where work happens.
- **The reframe:** instead of sandboxing agents, prepare them for the vicious world and let them keep working.
- **Proof 1:** keys stay in the vault; the agent gets a short-lived pass, never the key.
- **Proof 2:** one command reviews what the agent changed, runs four checks and gives two scores.
- **Proof 3:** register the personal apps that matter, then start, stop and read their API from one place.
- **The close:** every command can be run by talking to your main agent.
- **The address:** the GitHub mark, then `openwork-africa/ai2` sliding out of it to the right, with *Is your agent compromised?* beneath.

### 2. Scene sheet

| # | Scene | The job | On screen | Caption (verbatim) | Proof |
|---|---|---|---|---|---|
| 1 | Open | Set the promise | Generated rain clip; title fades in; dissolve to paper | *Software can finally be for everyone.* | — |
| 2 | Problem | Name the mistake | Caption only, typed | *We spend too much time containing agents, and not enough time letting them do work for us out where work happens.* | — |
| 3 | Vault | Keys never reach the agent | Four harness panels leak a key; terminal adds a vault; stat lands | *Any harness. Same leak.* / *Agents get a short-lived pass, never the key.* | 28.65M secrets on public GitHub, 2025 (GitGuardian) |
| 4 | Trained | Connect the leak to the model | One typed line on paper | *That is what AI is trained on.* | Same GitGuardian source |
| 5 | Review | One command checks the run | CISA stat; review command types; four check cards and two scores rise | *There's no time to check everything by hand.* / *One command. Four checks. Two scores.* | CISA KEV: 245 in 2025, 283 in 2026 to date |
| 6 | Apps | Register what matters | App-per-day stat; register command; five cards, Notes pins first | *One app a day. Thirty folders by the end of the month.* / *They get lost in all the folders.* / *Register the ones that matter.* | Arithmetic, labelled; ~1M projects/week on Lovable, self-reported |
| 7 | End | Recap the three tools | Vault, Security, Apps cards with their commands | *Don't sandbox agents. Double their stopping power with ai squared.* | — |
| 8 | Outro | Show the shortcut | Line types; honeycomb logo lands where the cursor blinks; pulses on half bars, wave through the letters | *Don't worry, all these commands you can run by talking to your main agent.* | — |
| 9 | GitHub | Leave the address | GitHub mark; repo path slides out to the right; tagline rises under it | *openwork-africa/ai2* / *Is your agent compromised?* | — |

### 3. Copy deck

- **Title:** *Software can finally be for everyone.*
- **Problem:** *We spend too much time containing agents, and not enough time letting them do work for us out where work happens.*
- **Harness:** *Any harness. Same leak.*
- **Vault:** *Agents get a short-lived pass, never the key.*
- **Trained:** *That is what AI is trained on.*
- **Review:** *There's no time to check everything by hand.* / *One command. Four checks. Two scores.*
- **Apps:** *One app a day. Thirty folders by the end of the month.* / *They get lost in all the folders.* / *Register the ones that matter.*
- **End:** *Don't sandbox agents. Double their stopping power with ai squared.*
- **Outro:** *Don't worry, all these commands you can run by talking to your main agent.*
- **Address:** *openwork-africa/ai2* / *Is your agent compromised?*
- **Naming:** in prose write **ai squared**, never the exponent form; use the logo for the mark. The full product name is **am I infected**. Write **Muse Code** with both words capitalised. The binary is `ai2`.
- **Voice:** lowercase, direct, first person where the script already is. No hype adjectives, no exclamation marks.

### 4. Facts, commands and sources

| On screen | Figure | Source | Caveat to respect |
|---|---|---|---|
| Secrets leaked | **28.65M** new hardcoded secrets on public GitHub in 2025, up 34% | GitGuardian, State of Secrets Sprawl 2026 | Count is for 2025 leaks only |
| Still valid | **70%** of secrets leaked in 2022 were still valid in Jan 2025; **above 64%** in Jan 2026 | GitGuardian, 2025 and 2026 editions | Never apply this rate to 2025's leaks; the cohort is 2022 |
| Exploited in the wild | **245** CISA KEV entries added in 2025; **283** in 2026 to date | CISA KEV catalog | Re-count from the live feed on render day; the 2026 number moves |
| Apps | **1 a day → 30 folders a month** | Arithmetic | Label it *arithmetic, not a survey* on screen |
| Apps at scale | **~1M** new projects a week, Lovable alone | Lovable, via press | Label *self-reported*; do not divide it into a per-user figure |

Store the URL, the exact quote and the snapshot date next to each source in `stats.ts`; the table above is only the on-screen label.

Rules:

- **Do not extrapolate.** No per-user-per-day figure exists; show the arithmetic as arithmetic and the platform figure as platform-level.
- **Quote the cohort exactly.** The 70% / 64% validity figures belong to the 2022 leaks.
- **Label self-reported data** on screen, in the source line.
- **Re-count the KEV feed on render day.**
- **No "exploited secrets" count exists.** Show the leak count and the still-valid share instead; say so in the design doc.

Commands, from the repo:

```
ai2 vault put --name stripe --approved-url https://api.stripe.com/
ai2 --run-path ./notes --objective "Add a search box"
ai2 apps register --name Notes --description "Plain notes you can search" --command "deno task start" --openapi /openapi.json --icon favicon.svg
```

- Read `docs/commands.md`, the CLI source and `--help` before typing a command.
- Match the real output text. The review card content is illustrative and is labelled *example* on screen.
- Never show a secret, token or key value. Secrets type as dots.

### 5. Look and motion

- **Sans face:** Geist Sans, from vercel.com/font. Vendor the woff2 files into `public/fonts`; load with `@remotion/fonts`.
- **Mono face:** Geist Mono, same source, for terminals, stat numbers and the repo path.
- **Font gate:** hold the render until Geist is loaded, so every caret, wrap and landing point is measured from the real face.
- **Type scale at 1920:** title 96/500, caption 64/500, stat 160 mono, stat label 40, source 24, terminal 36, card title 32, card body 24. Caption tracking −0.02em.
- **Typed caption:** one character at a time, 2 frames per character at 30 fps; pause 8 frames after a full stop, 4 after a comma, 6 after a colon; ±1 frame of seeded jitter; a block cursor in accent that blinks at 1 Hz only when done.
- **Follow camera:** scale 1.35×, cursor anchored 42% from the left and 50% from the top, translation eased over 4 frames per character. 12 frames after the last character, ease back to 1× over 24 frames, then hold.
- **Caption length:** never more than two lines at 64 px. If the line is longer, the script is wrong, not the component.
- **Terminals:** 1400 × 640, `#0E0E0E`, radius 16, padding 48, Geist Mono 36, no window chrome, no traffic lights. The command types; the output appears line by line, 6 frames apart, after a 10-frame beat. A success line uses accent. The camera follows the command only; zoom out after the output lands.
- **Palette:** paper `#FAFAFA`, ink `#111111`, ink-soft `#6B6B6B`, line `#E4E4E4`, terminal `#0E0E0E`, terminal text `#E8E8E8`, accent `#FFD400`, danger `#E5484D`, success `#2DB56B`, logo blue `#1397C1`.
- **Shape:** no gradients. One shadow value: `0 1px 2px rgba(0,0,0,.06)`. Cards and terminals share radius 16.
- **Motion:** one easing, `cubic-bezier(0.2, 0, 0, 1)`. Nothing bounces. Fades 12 frames, zoom-outs 24, scene cross-fades 10 on paper. No wipes, no slides, no stingers.
- **Cold open:** generated 1920 × 1080 clip, ~10 s, 30 fps. Locked-off camera, medium wide, person alone on a bench in steady rain, over-ear headphones, hood down, eyes closed, backlit by one sodium lamp. Calm, not sad. No umbrella, no phone glow, no storm, no text or logos. Generate four to six takes; reject warped hands, changing faces and camera drift. Title fades in at 1 s over 20 frames; a 6% ink overlay only while the title is in; the clip dissolves to paper at ~8 s; the first caption starts 10 frames after the paper is fully white.
- **Honeycomb:** the ai squared logo flies in on an arc with smear ghosts, lands where the cursor blinks and takes its place, then pulses on every half bar; each pulse sends rings out and a wave back through the letters.
- **GitHub lockup:** the mark pops in at centre, then glides left while `openwork-africa/ai2` slides out from behind it to the right, masked at the mark's edge. *Is your agent compromised?* rises out of a mask under the path. The lockup holds to the end while the bed fades.

### 6. Sound

- **Bed:** the 120 bpm shaker loop `rock-remix-120-shakers-x3.wav` from `/Users/rawkakani/Documents/agents/puppet/sandbox/app/beat/patterns/`, or its nine-pass cut. Copy it to `public/audio/bed.wav`; do not re-encode.
- **Tempo and grid:** at 120 bpm a beat is 15 frames and a bar is 60 frames at 30 fps. Every scene length is rounded to whole bars; every cut lands on a downbeat.
- **Levels:** −14 dB under the body, −20 dB under the opening so rain sits on top. 30-frame fade in, 60-frame fade out.
- **Rain:** separate track at −18 dB, fading out with the dissolve.
- **Clicks:** one soft sample per typed character at −24 dB, none for spaces; terminals use a quieter sample. If clicks fight the shakers, lower the clicks, never the bed.
- **Extras:** none. No voice-over, no stingers, no whooshes.

### 7. Deliverables

- **Master:** 1920 × 1080, 30 fps, H.264 mp4, CRF 16. `out/ai2-demo-trainer.mp4`.
- **Portrait:** 1080 × 1920, same timeline and copy, re-laid out for phone. `out/ai2-demo-trainer-portrait.mp4`.
- **X cut:** 1920 × 1080, with the cover held for at least 30 frames (1 s) before the film starts. `out/ai2-demo-trainer-x-cover.mp4`.
- **Covers:** plain Geist, no art. Landscape 1920 × 1080 and portrait 1080 × 1920. The cover reads `ai coding agent:` over `am i infected?`, centred as a block, both lines left-aligned, tight, sans.
- **Stills:** one per beat in `out/`, named by scene and frame.
- **Docs:** the design doc, the scene sheet, the source table.
- **Post copy:** a two-post thread, cover, alt text. Confirm the name and caption with the user; post only on explicit instruction.

### 8. Boundaries

- Mock the UI, never the facts. Illustrative card content is labelled *example*.
- Never print a real secret, token or key.
- One accent colour. No gradients, no new easing, no second font.
- Do not show a command that is not in the repo.
- Do not invent a per-user statistic by dividing a platform number.
- Do not ship a render with a caption clipped on a phone.
- Never post, submit or spend money on the user's behalf without approval.

## PART 2 — BUILD PROTOCOL

### Phase 0 — Ground truth (no code yet)

1. Read the repo: README, docs, CLI source, `docs/commands.md`. Run the commands' `--help`. Copy exact syntax and output; never invent a flag.
2. Research every number with a primary source. Record the exact wording, the cohort, the date and the URL. Put them in a table like §4.
3. Verify names and casing: product, binary, harnesses, repo handle, CTA.
4. Report a one-page source sheet. If a claim has no source, propose the honest replacement before scripting.

### Phase 1 — Script and beat sheet

1. Turn the brief into a scene sheet: one job per scene, one command or one idea.
2. Write every caption verbatim. Check each is two lines or fewer at 64 px.
3. Assign each scene a length in whole bars. State the total runtime.
4. Mark where the camera follows typing and where it zooms out.
5. Name what the viewer learns in each scene. Cut any scene that repeats another.
6. End with a table of frame ranges, actions and audio cues. This is the build contract.

### Phase 2 — Design doc (before code)

Write `docs/ai2-demo-animation.md` covering:

- Output table: composition, sizes, fps, length, codec, audio, font.
- Look: palette, type scale, shape, motion.
- The cold-open prompt, negative prompt, take selection and dissolve.
- The typed-caption spec with exact numbers.
- The mocked-UI inventory: harness panels, terminal, cards.
- The scene timing table in bars.
- Music, levels and fades.
- The stats table with sources and caveats.
- Portrait notes: what reflows, what stays identical.
- Build order.

No scene code until this exists. It is the thing every agent will work from.

### Phase 3 — Scaffold

- Remotion pinned at 4.0.538, React 19, TypeScript. Project in `demo/`.
- `src/theme.ts` is the only place tokens live: colours, type roles, `FPS`, `BAR`, `BEAT`, easing, portrait flag.
- Vendor the fonts. Add `FontGate`, which delays the render until Geist is loaded.
- Build the shared components: `Typed`/`TypedText`, `FollowCamera`/`LineCamera`, `Terminal`, `Card`, `Stat`, `Grain`.
- Make typing deterministic: a seeded schedule per line, so every render types the same frames.
- Register the full composition and a solo composition per scene in `Root`.
- `npx tsc --noEmit` must pass before any scene work starts.

### Phase 4 — Build scene by scene

- Split the sheet into groups. Use one agent per group, with disjoint files. Freeze `theme.ts`, shared components and `Root` while agents run; integrate yourself.
- Brief each agent with: the exact copy, the frame budget, the tokens, the shared components, the entrance and exit, the beat map, and the acceptance checks.
- Each agent must: typecheck; render stills at every beat, including the first and last frame; read the images; report frame numbers and any deviation from the brief.
- Reject work that introduces a new colour, size, easing, font or shared-file edit.
- Integrate in `Root` with `<Series>`; each scene keeps its own internal timings so re-ordering never breaks it.
- Build order: opening clip, problem, vault, trained, review, apps, end, outro, address. Start the clip generation first; it queues.
- Every scene after the opening starts and ends on blank paper.

### Phase 5 — Join and polish

- Join with `<Series>` and a 10-frame cross-fade on paper.
- Put the bed under frame 0; make every cut land on a downbeat.
- Key the clicks to the typed-character frames. Duck nothing; lower the clicks if they fight.
- Sync the outro pulse to the half bar: rings out, wave through the letters.
- Watch the full film once at speed and once at half speed, the second time only at the caret.
- Check the first scene after the opening for stutter and for a caret that lands where typing begins. Fix the cause (font load, measure timing), not the copy.
- Confirm no scene jumps: blank paper in, blank paper out.

### Phase 6 — Variants

- **Portrait:** branch layout on a `PORTRAIT` flag, same copy and timeline. Re-flow the chat panels, stack the cards, widen the type. Verify the landscape cut is byte-identical at `--concurrency=1` before accepting the portrait change.
- **X cut:** hold the cover for 1 s at the head; check the first frame in the actual upload, not the composer preview.
- **Covers:** plain Geist, both orientations, no art.
- **Other sizes:** only if the brief asks; keep the same lockups.

### Phase 7 — QA gate

Do not deliver until all of these pass:

- [ ] `npx tsc --noEmit` is clean.
- [ ] Full render completes; duration and fps match the sheet.
- [ ] Every scene's first and last frame is blank paper.
- [ ] Every caption fits in two lines and is unclipped in portrait.
- [ ] Every stat carries its visible source line; no extrapolated figure.
- [ ] Every command matches the repo; the review cards are labelled example.
- [ ] No secret or token appears in any frame.
- [ ] The caret lands where typing starts; no stutter after the opening.
- [ ] Audio: bed level, fades, no clipping, clicks not fighting.
- [ ] The cover holds for 1 s in the X cut.
- [ ] Files are named and placed per §7; README has the render commands.

Run baseline comparisons at `--concurrency=1`. Parallel renders can flake on font timing; if a caret is one character early, suspect the font gate, not the copy.

### Phase 8 — Deliver

- Put the master, portrait, X cut, covers and stills in `out/`.
- Keep the design doc, the scene sheet and the source table in `docs/`.
- Write a short README: how to preview, render, and re-cut.
- If asked for post copy: a two-post thread, the cover, alt text. Confirm the name and the caption with the user; post only on explicit instruction.

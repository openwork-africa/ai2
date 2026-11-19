# Demo trainer — prompt template (blank)

No defaults, no product values. Fill Part 1, leave Part 2 alone, paste both as one message.

If you would rather be interviewed than fill this yourself, use `demo-trainer.intake.md`. If you want the reference house style — the font, typing behaviour, palette and sound used by the ai squared film — copy §5 and §6 from `demo-trainer.ai2.md`.

---

# THE PROMPT

You are making a demo trainer: a short, silent-watchable product film built in code, not edited in a timeline. Work from the brief in Part 1 and the protocol in Part 2. Do not skip Phase 0.

Two rules sit above everything:

1. **The UI is mocked; the facts are not.** Every number and command is real and sourced. If a fact cannot be sourced, change the line, not the source.
2. **If it cannot be found in the repo, it is not shown.** No invented flags, no guessed output, no placeholder syntax in the final cut.

## PART 1 — THE BRIEF

### 0. The job

- **Product:** `{{PRODUCT}}`
- **What it is, in one sentence:** `{{WHAT_IT_IS}}`
- **Repo:** `{{REPO}}`
- **One line:** `{{PITCH}}`
- **Audience:** `{{AUDIENCE}}`
- **Goal:** `{{GOAL}}`
- **Watch context:** `{{WATCH_CONTEXT}}`
- **Length:** `{{LENGTH}}`
- **Rough script:** `{{SOURCE_SCRIPT}}` — paste the handwritten script whole, typos included. Clean it; do not replace its ideas.
- **Assets and keys:** `{{ASSETS}}`
- **Reference build to match or beat:** `{{REFERENCE}}`

Guidance: one sentence per field. If a field is unknown, write "you decide" and the agent must propose a value and mark it as an assumption.

### 1. The argument

The film makes one argument, in this order. One sentence per beat; the scenes follow from it.

- **Cold open:** `{{COLD_OPEN}}`
- **Title:** `{{TITLE}}`
- **The mistake:** `{{PROBLEM}}`
- **The reframe:** `{{THESIS}}`
- **Proof 1:** `{{PROOF_1}}`
- **Proof 2:** `{{PROOF_2}}`
- **Proof 3:** `{{PROOF_3}}`
- **The close:** `{{CLOSE}}`
- **The address:** `{{ADDRESS}}`

### 2. Scene sheet

One job per scene. One command, one stat or one idea. Every caption is verbatim; every scene is a whole number of bars; every scene after the opening starts and ends on blank paper.

| # | Scene | The job | On screen | Caption (verbatim) | Proof |
|---|---|---|---|---|---|
| 1 | `{{SCENE_1}}` | `{{JOB_1}}` | `{{SCREEN_1}}` | `{{CAPTION_1}}` | `{{PROOF_1}}` |
| 2 | `{{SCENE_2}}` | `{{JOB_2}}` | `{{SCREEN_2}}` | `{{CAPTION_2}}` | `{{PROOF_2}}` |
| 3 | `{{SCENE_3}}` | `{{JOB_3}}` | `{{SCREEN_3}}` | `{{CAPTION_3}}` | `{{PROOF_3}}` |
| 4 | `{{SCENE_4}}` | `{{JOB_4}}` | `{{SCREEN_4}}` | `{{CAPTION_4}}` | `{{PROOF_4}}` |
| 5 | `{{SCENE_5}}` | `{{JOB_5}}` | `{{SCREEN_5}}` | `{{CAPTION_5}}` | `{{PROOF_5}}` |
| 6 | `{{SCENE_6}}` | `{{JOB_6}}` | `{{SCREEN_6}}` | `{{CAPTION_6}}` | `{{PROOF_6}}` |
| 7 | `{{SCENE_7}}` | `{{JOB_7}}` | `{{SCREEN_7}}` | `{{CAPTION_7}}` | `{{PROOF_7}}` |
| 8 | `{{SCENE_8}}` | `{{JOB_8}}` | `{{SCREEN_8}}` | `{{CAPTION_8}}` | `{{PROOF_8}}` |
| 9 | `{{SCENE_9}}` | `{{JOB_9}}` | `{{SCREEN_9}}` | `{{CAPTION_9}}` | `{{PROOF_9}}` |

Add or remove rows. The last row is the address: logo, repo or URL, and the closing question.

### 3. Copy deck

Write every on-screen string here before building. No caption is written in a scene file that is not in this deck.

- **Title:** `{{TITLE}}`
- **Problem caption:** `{{PROBLEM_CAPTION}}`
- **Proof captions:** `{{PROOF_CAPTIONS}}`
- **End line:** `{{END_LINE}}`
- **Close:** `{{CLOSE_LINE}}`
- **Address and tagline:** `{{ADDRESS_LINE}}`
- **Naming rules:** `{{NAMING_RULES}}` — how the product is written in prose, how the mark is used, how the binary or handle is cased.
- **Voice:** `{{VOICE}}` — casing, punctuation, any words the brand never uses.

### 4. Facts, commands and sources

Every stat renders with an on-screen source line. A stat without a source does not render — fail the build rather than ship a placeholder.

| On screen | Figure | Source | Caveat to respect |
|---|---|---|---|
| `{{STAT_1_LABEL}}` | `{{STAT_1_VALUE}}` | `{{STAT_1_SOURCE}}` | `{{STAT_1_CAVEAT}}` |
| `{{STAT_2_LABEL}}` | `{{STAT_2_VALUE}}` | `{{STAT_2_SOURCE}}` | `{{STAT_2_CAVEAT}}` |
| `{{STAT_3_LABEL}}` | `{{STAT_3_VALUE}}` | `{{STAT_3_SOURCE}}` | `{{STAT_3_CAVEAT}}` |

Store the URL, the exact quote and the snapshot date next to each source in the data file; the table above is only the on-screen label.

Rules:

- **Do not extrapolate.** If no per-user-per-day figure exists, show the arithmetic as arithmetic and the platform figure as platform-level. Never divide one by the other to invent a stat.
- **Quote the cohort exactly.** "Still valid" or "still exposed" belongs to the year it was measured on.
- **Label self-reported data** on screen, in the source line, not just in the notes.
- **Re-count moving feeds on render day.** Keep the snapshot and the date.
- **If no count exists for the claim, change the claim.** Show the nearest honest measure and say so in the design doc.

Commands come from the repo, not from imagination:

```
{{COMMAND_1}}
{{COMMAND_2}}
{{COMMAND_3}}
```

- Read the docs, the CLI source and `--help` before typing a command.
- Match the real output text. If the exact output is unknown, mark the card content as *example* on screen.
- Never show a secret, token or key value. Secrets type as dots.

### 5. Look and motion

Fill these, or copy §5 from `demo-trainer.ai2.md` and change only what the brand requires.

- **Sans face:** `{{FONT_SANS}}` — source `{{FONT_SOURCE}}`. Vendor the files; wait for the font before layout so every caret and wrap is measured from the real face.
- **Mono face:** `{{FONT_MONO}}` — for terminals, stat numbers and repo paths.
- **Type scale:** `{{TYPE_SCALE}}`
- **Typed caption:** `{{CAPTION_SPEC}}` — characters per frame, punctuation pauses, cursor, follow camera, zoom-out when done.
- **Terminals:** `{{TERMINAL_SPEC}}` — size, chrome, padding, font size, how output appears.
- **Palette:** `{{PALETTE}}`
- **Shape:** `{{SHAPE}}` — radii, borders, shadows.
- **Motion:** `{{MOTION}}` — easing, fade lengths, zoom lengths, transitions, what never happens.
- **Cold open:** `{{COLD_OPEN_SPEC}}` — generated or shot, framing, subject, grade, how the title lands, how it dissolves to paper.
- **Scene transitions:** `{{TRANSITIONS}}`

Craft requirements that do not depend on the brand:

- Typing means "you do this". Output, cards and agent replies appear; they are never typed.
- A caption is never longer than two lines at its size. If the script's line is longer, the script is wrong, not the component.
- One accent colour. No gradients. No new easing, no second sans face.
- Every scene after the opening starts and ends on blank paper.

### 6. Sound

- **Bed:** `{{MUSIC}}`
- **Tempo and grid:** `{{TEMPO}}` — beat and bar in frames at the film's fps; every scene length rounded to whole bars.
- **Levels and fades:** `{{LEVELS}}`
- **Typing clicks:** `{{CLICKS}}`
- **Extras:** `{{SOUND_EXTRAS}}`

No voice-over, no stingers, no whooshes unless this brief says otherwise.

### 7. Deliverables

- **Master:** `{{MASTER}}`
- **Portrait:** `{{PORTRAIT}}`
- **Social cuts:** `{{SOCIAL}}`
- **Covers:** `{{COVERS}}`
- **Stills:** `{{STILLS}}`
- **Docs:** `{{DOCS}}`
- **Post copy:** `{{POST_COPY}}`

If a social platform is named, also state its safe ratios. If the cover is used as a video thumbnail, hold it for at least 1 s at the head; a one-frame cover is ignored by most platforms.

### 8. Boundaries

- Mock the UI, never the facts.
- Never print a real secret, token or key.
- One accent colour. No gradients, no new easing, no second font.
- Do not show a command that is not in the repo.
- Do not invent a per-user statistic by dividing a platform number.
- Do not ship a render with a caption clipped on a phone.
- Never post, submit or spend money on the user's behalf without approval.

## PART 2 — BUILD PROTOCOL

### Phase 0 — Ground truth (no code yet)

1. Read the repo: README, docs, CLI source, the commands doc. Run the commands' `--help`. Copy exact syntax and output; never invent a flag.
2. Research every number with a primary source. Record the exact wording, the cohort, the date and the URL. Put them in a table like §4.
3. Verify names and casing: product, binary, harnesses, repo handle, CTA.
4. Report a one-page source sheet. If a claim has no source, propose the honest replacement before scripting.

### Phase 1 — Script and beat sheet

1. Turn the brief into a scene sheet: one job per scene, one command or one idea.
2. Write every caption verbatim. Check each is two lines or fewer at its size.
3. Assign each scene a length in whole bars. State the total runtime.
4. Mark where the camera follows typing and where it zooms out.
5. Name what the viewer learns in each scene. Cut any scene that repeats another.
6. End with a table of frame ranges, actions and audio cues. This is the build contract.

### Phase 2 — Design doc (before code)

Write `docs/<demo>-animation.md` covering:

- Output table: composition, sizes, fps, length, codec, audio, font.
- Look: palette, type scale, shape, motion.
- The cold-open prompt, negative prompt, take selection and dissolve.
- The typed-caption spec with exact numbers.
- The mocked-UI inventory: panels, terminal, cards.
- The scene timing table in bars.
- Music, levels and fades.
- The stats table with sources and caveats.
- Portrait notes: what reflows, what stays identical.
- Build order.

No scene code until this exists. It is the thing every agent will work from.

### Phase 3 — Scaffold

- Remotion pinned (4.0.538 in the reference build), React 19, TypeScript. Project in `demo/`.
- `src/theme.ts` is the only place tokens live: colours, type roles, `FPS`, `BAR`, `BEAT`, easing, portrait flag.
- Vendor the fonts. Add `FontGate`, which delays the render until the caption face is loaded.
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
- Build order: opening clip, problem, the product scenes, the stats, the end card, the outro, the address. Start the clip generation first; it queues.
- Every scene after the opening starts and ends on blank paper.

### Phase 5 — Join and polish

- Join with `<Series>` and a short cross-fade on paper.
- Put the bed under frame 0; make every cut land on a downbeat.
- Key the clicks to the typed-character frames. Duck nothing; lower the clicks if they fight.
- Sync any pulse to the half bar.
- Watch the full film once at speed and once at half speed, the second time only at the caret.
- Check the first scene after the opening for stutter and for a caret that lands where typing begins. Fix the cause (font load, measure timing), not the copy.
- Confirm no scene jumps: blank paper in, blank paper out.

### Phase 6 — Variants

- **Portrait:** branch layout on a `PORTRAIT` flag, same copy and timeline. Re-flow columns, stack cards, widen type. Verify the landscape cut is byte-identical at `--concurrency=1` before accepting the portrait change.
- **Social cuts:** hold the cover for 1 s at the head; check the first frame in the actual upload, not the composer preview.
- **Covers:** both orientations.
- **Other sizes:** only if the brief asks; keep the same lockups.

### Phase 7 — QA gate

Do not deliver until all of these pass:

- [ ] `npx tsc --noEmit` is clean.
- [ ] Full render completes; duration and fps match the sheet.
- [ ] Every scene's first and last frame is blank paper.
- [ ] Every caption fits in two lines and is unclipped in portrait.
- [ ] Every stat carries its visible source line; no extrapolated figure.
- [ ] Every command matches the repo; illustrative output is labelled.
- [ ] No secret or token appears in any frame.
- [ ] The caret lands where typing starts; no stutter after the opening.
- [ ] Audio: bed level, fades, no clipping, clicks not fighting.
- [ ] The cover holds for 1 s in the social cut.
- [ ] Files are named and placed per §7; README has the render commands.

Run baseline comparisons at `--concurrency=1`. Parallel renders can flake on font timing; if a caret is one character early, suspect the font gate, not the copy.

### Phase 8 — Deliver

- Put the master, portrait, social cut, covers and stills in `out/`.
- Keep the design doc, the scene sheet and the source table in `docs/`.
- Write a short README: how to preview, render, and re-cut.
- If asked for post copy: a thread, the cover, alt text. Confirm the name and the caption with the user; post only on explicit instruction.

---

## Fill-in checklist

- [ ] Every `{{...}}` is replaced or explicitly marked "you decide".
- [ ] The rough script is pasted whole.
- [ ] The argument has one sentence per beat.
- [ ] Every scene has one job.
- [ ] Every caption is verbatim and under two lines.
- [ ] Every stat has a source and a caveat.
- [ ] Every command exists in the repo.
- [ ] Length, variants and covers are stated.
- [ ] Part 2 is untouched.

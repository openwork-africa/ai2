# ai² demo trainer animation · production design

How the animation is made: the opening video, the typed captions, the mocked terminals and cards, and the Remotion build that puts it together. The script it follows is at the end.

Status 2026-11-03: design only. Nothing here is built yet. The three statistics are verified and sourced in §7.

---

## 1. Output

| | |
|---|---|
| Composition | `Ai2DemoTrainer` |
| Size | 1920 × 1080, 30 fps (a 1080 × 1920 portrait cut is listed in §9) |
| Length | about 95 s (per-scene timings in §6) |
| Codec | H.264 mp4, `--crf 16`, plus a ProRes master for re-cuts |
| Audio | one music bed under everything: the shaker version of the rock remix, 120 bpm, the three-pass loop (`/Users/rawkakani/Documents/agents/puppet/sandbox/app/beat/patterns/rock-remix-120-shakers-x3.wav`); rain on top in scene 1; soft key clicks under typing; no voice-over. Details in §6a. |
| Font | Geist Sans for every caption and title, Geist Mono for every terminal, from vercel.com/font (files vendored into `remotion/public/fonts`, loaded with `@remotion/fonts`) |

The thing being trained is the eye: every scene teaches one command by showing it typed, so the viewer has watched the command before they ever run it.

---

## 2. Look

One system across all scenes. These are the only values the compositions use.

**Palette**

| Token | Value | Use |
|---|---|---|
| `paper` | `#FAFAFA` | the white screen every scene after the intro sits on |
| `ink` | `#111111` | captions, terminal text |
| `ink-soft` | `#6B6B6B` | source lines under stats, prompts in the terminal |
| `line` | `#E4E4E4` | card and terminal borders |
| `terminal` | `#0E0E0E` | terminal background |
| `terminal-text` | `#E8E8E8` | terminal output |
| `accent` | `#FFD400` | one accent, for the ✓ on success, the ai² mark and the cursor in captions (same yellow as the Worklyn still covers) |
| `danger` | `#E5484D` | the exposed secret in scene 3a, a single use |

No gradients anywhere. Flat paper, flat cards, one shadow value (`0 1px 2px rgba(0,0,0,.06)`) for cards lifted off the paper.

**Type scale (at 1920 wide)**

| Role | Font | Size | Weight | Tracking |
|---|---|---|---|---|
| Title (scene 1) | Geist Sans | 96 | 500 | -0.02em |
| Caption | Geist Sans | 64 | 500 | -0.02em |
| Stat number | Geist Mono | 160 | 500 | -0.04em |
| Stat label | Geist Sans | 40 | 400 | 0 |
| Source line | Geist Sans | 24 | 400 | 0, `ink-soft` |
| Terminal | Geist Mono | 36 | 400 | 0, line-height 1.5 |
| Card title | Geist Sans | 32 | 500 | -0.01em |
| Card body | Geist Sans | 24 | 400 | 0 |

**Shape.** Terminals and cards use one radius, 16 px. The terminal has no window chrome: no traffic lights, no title bar, just a dark rounded panel with 48 px padding. A thin `line` border on cards, none on the terminal.

**Motion.** One easing for everything that moves, `cubic-bezier(0.2, 0, 0, 1)` (Remotion `Easing.bezier(0.2, 0, 0, 1)`). Nothing bounces. Fades are 12 frames. Zoom-outs are 24 frames. Scene transitions are a 10-frame cross-fade on paper; no wipes, no slides.

---

## 3. The opening video (scene 1)

The person in the rain is a generated clip, made with Runway (Gen-4) or Muse, never a stock clip and never a code-drawn scene. Generate at 1920 × 1080, 10 s, 24 or 30 fps, then conform to 30 fps in the render.

**Generation prompt**

> A person sits alone on a wooden bench in a city park at dusk, in steady rain, wearing over-ear headphones and a dark raincoat with the hood down. They are still, eyes closed, head tilted slightly back, listening. Rain falls in fine straight lines, lit from behind by one sodium street lamp so each drop catches the light. Puddles on the path reflect the lamp. Camera: locked off, medium wide shot at eye level, slight anamorphic bokeh in the background from distant traffic lights, no camera movement. Film look: 35 mm, soft contrast, muted greens and amber, grain visible, shallow depth of field with the person sharp. No text, no logos, no other people, no umbrella. The mood is calm, not sad: someone content in bad weather.

**Negative / avoid:** umbrella, phone screen glow, lightning, dramatic storm, crowds, text, watermark, camera push-in, slow-motion rain.

**Why these choices.** Still camera so the title can sit on the frame without fighting movement. Backlit rain so the drops read at a glance. Eyes closed so the person is clearly listening, which is the "music" of the brief without showing a device. Calm mood because the title that lands on it, *Software can finally be for everyone*, is hopeful.

**Selection.** Generate four to six takes. Pick on: the person is sharp, the rain is visible against the dark, nothing moves except rain and water. Reject any take with warped hands or a face that changes between frames.

**Grade in Remotion.** Lay a 6 % `ink` overlay for the first 20 frames while the title fades in, so the white Geist title has contrast. No vignette, no colour change: the grade is in the generation.

**How it ends.** At 8.0 s the clip starts a 30-frame fade to `paper`. The white screen is not a cut; the rain dissolves into the paper and the first caption begins typing on it 10 frames after the paper is fully white. That one dissolve is the whole transition from "world" to "the trainer".

---

## 4. The typed caption

Every caption, stat and command in scenes 2 to end is typed. One component, `<Typed>`, does it, so the feel is identical everywhere.

**Behaviour**

1. Text appears one character at a time. Base speed 2 frames per character (15 chars/s). Punctuation pauses: 8 extra frames after `.`, 4 after `,`, 6 after `:`. Spaces are not paused. Jitter of ±1 frame per character from a seeded random so the rhythm is human, and the same on every render.
2. A block cursor, `accent` for captions and `terminal-text` for terminals, sits after the last character and blinks at 1 Hz once typing is done. While typing it does not blink.
3. **Camera follows the cursor.** The whole caption layer is scaled to 1.35× and translated so the cursor sits at a fixed anchor point, 42 % from the left and 50 % from the top of the frame. As the text grows the translation moves (eased per character, 4 frames) and the viewer's eye stays on the anchor. In practice: the text slides left under a steady cursor, like a typewriter carriage.
4. **Zoom out when done.** 12 frames after the last character, the layer eases from 1.35× to 1.0× and the translation to zero over 24 frames, with the same easing as everything else. The caption now sits centred and whole. Hold for the scene's listed hold time, then the scene ends.
5. Key clicks: one soft click sample per character (three samples rotated, −24 dB), none for spaces. Muted inside terminals, which have their own quieter sample.

**Implementation.** `<Typed text start speed anchor>` returns `{ visibleText, cursorX, done }` from `useCurrentFrame()`. Measure character advance with a hidden canvas `measureText` for Geist at the composition size, once per text, memoised; this gives the exact cursor X for the follow translation without a DOM read. For multi-line captions, wrap at 1400 px and the anchor follows the cursor's line too (vertical translation).

**Rule.** Captions are never longer than two lines at 64 px. If a line in the script is longer, the script is wrong, not the component.

---

## 5. Mocked UI

The screens are mock-ups, built as Remotion components, not screenshots. The numbers on them are real (see §7). Three kinds.

### 5.1 Harness conversations (scene 3a)

Four columns, each a chat panel 420 × 760 on paper, 24 px gap, labelled at the top in `ink-soft` 24 px: **Claude Code**, **Codex**, **Cursor**, **Muse Code**. The label is the only brand reference; the panels themselves are neutral (no logos, no product colours) so the point reads as "any harness". Muse Code is Meta's agent (`muse`, the same one listed in the Academy tools); it is included so the row covers the harnesses ai² already supports over MCP and the ones a viewer is likely to have open.

Each panel plays the same four-message exchange, staggered by 12 frames per column:

```
user   › here's the key: sk_live_4e••••••••••••••••••
agent  › Please don't paste secrets here. I'll use it, but it will sit in this transcript.
agent  › Reading .env …
agent  › STRIPE_KEY=sk_live_4e5f… (sent to tool: fetch https://…)
```

The last line is where `danger` is used: the exposed key's text turns `danger` over 6 frames and stays. Messages fade in over 8 frames; no typing inside the panels (typing is reserved for the captions and terminals, so it means "you do this").

### 5.2 The terminal (scenes 3b, 4, 5)

One component, `<Terminal>`, 1400 × 640 centred, `terminal` background, 16 px radius, 48 px padding, Geist Mono 36.

- Prompt `$ ` in `ink-soft`, command typed with `<Typed>` in `terminal-text`.
- 10 frames after the command finishes, the output appears line by line, 6 frames apart, not typed (output is not something the viewer does).
- A `✓` line is `accent`.
- A secret prompt shows dots that type at the same rate as text: `Secret: ••••••••••••••••`.
- The camera-follow applies to the command only; the zoom-out happens after the output has fully appeared.

### 5.3 Cards

**Security cards (scene 4).** Four cards for the four checks the review runs: *security-audit*, *dependency-audit*, *patch-sweep*, *owasp-audit*, plus two score cards above them (the review gives two scores, per the README). Cards are 440 × 260, paper on paper with a `line` border, title in Geist Sans 32, a one-line body, and a `StatusDot` (green `#2DB56B` pass, `accent` fixed, `danger` found). They appear inside the terminal panel, replacing its text: the terminal's output fades out, the cards rise 24 px into place with a 6-frame stagger. Content of the cards is illustrative of what a review prints; it is not a real review result and is labelled "example" at 20 px in the corner of the panel.

**App cards (scene 5).** The Apps page as cards: icon (a plain glyph, no real product icon), name, description, a **Running** `StatusDot`, and a small `openapi` tag. Five cards in a row, the registered one (*Notes*) enters last and lands in the first slot while the others shift right, so "pinning" is literally shown.

**ai² cards (end).** The three product cards, **Vault · Security · Apps**, each with its command underneath in Geist Mono 24, on paper. These are the only cards with the `accent` mark in the corner.

---

## 6. Scenes and timing

Frame counts at 30 fps. "Type" = typing duration from the `<Typed>` rules, rounded; "Hold" = after the zoom-out.

| # | Scene | Starts | Length | What happens |
|---|---|---|---|---|
| 1 | Open | 0:00 | 10.0 s | Rain clip. Title fades in at 1.0 s over 20 frames, holds, fades out with the clip at 8.0 s into paper. |
| 2 | The problem | 0:10 | 9.0 s | Caption types (two lines, ~7 s), zoom-out, hold 1.5 s. |
| 3a | Any harness, same leak | 0:19 | 10.0 s | Four panels fade in, exchange plays (4.5 s), `danger` lands, caption "Any harness. Same leak." types below (2.5 s), zoom-out, hold 2 s. |
| 3b | Add a vault | 0:29 | 11.0 s | Panels dissolve, terminal in. `ai2 vault put …` types (5 s), secret dots (1 s), ✓ output, zoom-out. Caption "Agents get a short-lived pass, never the key." types under the terminal (3 s). Hold 1.5 s. |
| 3c | Stat · secrets | 0:40 | 9.0 s | Terminal dissolves. **28.65M** counts up in Geist Mono 160 over 30 frames while the label "secrets leaked on public GitHub in 2025" types; a second line types: "70% of those leaked in 2022 were still valid three years on". Source line fades in. Hold 3 s. |
| 4a | Stat · exploits | 0:49 | 8.0 s | Two numbers side by side, **245** and **283**, count up under the labels "2025" and "2026 so far"; label "known exploited vulnerabilities (CISA KEV)" types; source line. Caption "There's no time to check everything by hand." types. |
| 4b | Review | 0:56 | 14.0 s | Terminal in. `ai2 --run-path ./notes --objective "Add a search box"` types (6 s). Output: "Checking the stack against exploits published in the last few days…", then the terminal text fades and the six security cards rise (3 s). Caption "One command. Four checks. Two scores." types. Zoom-out, hold 2 s. |
| 5a | Stat · apps | 1:10 | 10.0 s | Line one types: "One app a day." then **30** counts up with the label "folders by the end of the month"; source line *arithmetic, not a survey*. Line two, 2 s later: **~1M** counts up, label "new projects a week, on Lovable alone" types, source line "Lovable, self-reported, 2026". Caption "They get lost in all the folders." types. |
| 5b | Register | 1:20 | 10.0 s | Terminal. `ai2 apps register --name Notes …` types (7 s; this is the longest command, let it run, it is the lesson). ✓ output. |
| 5c | Pinned | 1:30 | 6.0 s | Terminal dissolves, five app cards, Notes lands in slot one. Caption "Register the ones that matter." types. |
| End | Card | 1:36 | 8.0 s | Three ai² cards fade in, caption types: "Don't sandbox agents. Double their stopping power with ai²." Zoom-out to the full card row. Hold 3 s, fade to paper. |

Total ≈ 1:44. If it needs to be under 90 s, cut 3c into 3b (stat under the terminal) and 5a into 5b.

### 6a. Music

The bed is `rock-remix-120-shakers-x3.wav`, the three-pass shaker loop from the puppet beat patterns. It plays under the whole piece, from frame 0 to the final fade, and it sets the tempo of everything else: at 120 bpm a beat is 15 frames and a bar is 60 frames, so the timings in §6 are rounded to bars where they can be, and scene changes land on a downbeat.

- Copy it to `public/audio/bed.wav`; do not re-encode. It is 12.6 MB, so a `.wav` is fine for the render and `<Audio>` streams it.
- Level: −14 dB under scenes 2 to end, −20 dB under scene 1 so the rain sits on top, 30-frame fade-in at the start, 60-frame fade-out under the end card.
- The loop is already three passes long; if the cut runs past it, `loop` the `<Audio>` and trim the overlap at a bar boundary (a bar is exactly 2.0 s).
- Key clicks sit at −24 dB over it and are ducked by nothing: the shakers and the clicks live in the same range, so if they fight, lower the clicks, never the bed.
- Scene 1 rain is a separate track, −18 dB, fades out with the dissolve to paper.

Nothing else is added to the audio. No stingers, no whoosh on transitions, no voice.

---

## 7. The numbers

The UI is mocked; the numbers are not. Each stat renders from `remotion/src/data/stats.ts` and that file carries the source for each figure. A stat with no source does not render: the composition throws at build time, so a placeholder can never ship.

```ts
export type Stat = { value: number; label: string; source: { name: string; url: string; year: number } }
```

Verified 2026-11-03. Each figure is checked against the publisher's own page or feed.

| Scene | On screen | Figure | Source |
|---|---|---|---|
| 3c | **28.65M** secrets leaked on public GitHub in 2025, up 34% | GitGuardian, State of Secrets Sprawl 2026 | https://dev.to/gitguardian/the-state-of-secrets-sprawl-2026-ai-service-leaks-surge-81-and-29m-secrets-hit-public-github-2bgj |
| 3c (second line) | **70%** of secrets leaked in 2022 were still valid in Jan 2025; still above 64% in Jan 2026 | GitGuardian 2025 and 2026 reports | https://blog.gitguardian.com/the-state-of-secrets-sprawl-2025/ |
| 4a | Known exploited vulnerabilities added to CISA KEV: **245** in 2025, **283** in 2026 so far (to 3 Nov) | CISA KEV catalog | https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json |
| 5a (first line) | **1** app a day, **30** folders by the end of the month | arithmetic, labelled as such on screen | none; it is true by construction |
| 5a (second line) | **~1M** new projects a week on Lovable alone | Lovable, self-reported, via press | https://aidirectory.com/news/lovable-500m-annualized-revenue-1m-new-projects-a-week |

What the research changed in the script:

- **Scene 3c is "exposed", not "used".** No publisher counts leaked secrets that were actually exploited. The scene shows the leak count and the still-valid share instead; "still valid" is the honest version of "used". Do not say "70% of 2025's leaks are valid": the 70% is measured on 2022 leaks only.
- **Scene 4a uses CISA KEV, not raw CVE counts.** KEV is the official "exploited in the wild" list, so it matches the scene's meaning. Raw CVE counts exist (2025: 48,185 per Jerry Gamblin; 2026 to date about 93,000 by NVD API, method-dependent) but would need a method note on screen, so they stay out. The 2026 KEV number must be re-counted from the feed on render day, since it moves.
- **Scene 5a is arithmetic first, stat second.** No per-user-per-day figure exists from Lovable, Bolt, Replit or v0, and dividing Lovable's 1M a week by its user base would give under one per person per day, which says the opposite of the scene. So the first line is arithmetic, "One app a day. Thirty folders by the end of the month.", with the source line reading *arithmetic, not a survey*. The Lovable number is the second line, proving apps are generated at scale, labelled self-reported. The caption "They get lost in all the folders." lands after both.

The 2024 figure, 23.8M (+25%), is confirmed on GitGuardian's blog if an older-year comparison is ever wanted. The CISA feed snapshot used for the count is kept at `/private/tmp/claude-501/-Users-rawkakani-Documents-openwork/148e4a84-e601-441b-9b8e-6b0b149ff77c/scratchpad/kev.json`.

---

## 8. Project layout

```
ai2/demo/
  package.json              remotion 4.x, @remotion/fonts, @remotion/media-utils
  remotion.config.ts        points Chrome at Playwright's headless shell (see tests/demo history; Remotion's own Chrome dies)
  public/
    fonts/Geist*.woff2, GeistMono*.woff2
    video/rain.mp4          the selected Runway/Muse take, conformed
    audio/bed.wav           rock-remix-120-shakers-x3.wav, copied as is
    audio/rain.wav, click-1..3.wav, click-term.wav
  src/
    Root.tsx                registers Ai2DemoTrainer (1920×1080) and Ai2DemoTrainerPortrait
    theme.ts                the tokens in §2, nothing else defines a colour or size
    data/
      stats.ts              §7
      script.ts             every caption and command string, in order, with scene ids
    components/
      Typed.tsx             §4
      FollowCamera.tsx      the scale/translate wrapper driven by Typed's cursorX
      Terminal.tsx
      ChatPanel.tsx
      Stat.tsx              count-up number + typed label + source line
      Card.tsx, StatusDot.tsx
      Paper.tsx             the page background and cross-fade between scenes
    scenes/
      01-Open.tsx … 09-End.tsx
    audio/
      clicks.ts             maps Typed character frames to click samples
```

Sequencing: `Root` lays scenes out with `<Series>`, each `<Series.Sequence>` sized from the §6 table, and `<TransitionSeries>` with a 10-frame `fade()` between them. Each scene file owns its own internal timings as offsets from its first frame, so re-ordering scenes never breaks them.

Build: `npm run render` → `out/ai2-demo-trainer.mp4`; `npm run still -- --frame 1200` for review stills; `npm run preview` for the studio.

---

## 9. Portrait cut

`Ai2DemoTrainerPortrait` at 1080 × 1920 reuses every scene with the layout props flipped: the four chat panels sit two by two, the terminal is 960 wide with Geist Mono 30, cards wrap two across, captions are 56 px. Type and timings are unchanged. The rain clip is regenerated at 1080 × 1920 with the same prompt plus "vertical framing, the bench centred, more sky above".

---

## 10. Build order

1. Vendor Geist, write `theme.ts`, `Typed.tsx` and `FollowCamera.tsx`. Render scene 2 alone and judge the typing feel; adjust speed and pauses here, once, before any other scene exists.
2. Generate the rain takes (Runway or Muse), pick one, build scene 1 and the dissolve into scene 2.
3. `Terminal.tsx` and scene 3b. Confirm the command output text against `docs/commands.md` so the mock prints what the real binary prints.
4. `ChatPanel.tsx` and scene 3a.
5. `Stat.tsx` with the first verified figure; the build must fail without a source.
6. Cards, scenes 4b, 5c, End.
7. Audio: the shaker bed under everything per §6a, rain over it in scene 1, clicks keyed to `Typed`. Check that scene changes land on bars; nudge the §6 timings, not the music.
8. Full render, watch once at speed, once at half speed looking only at the cursor follow. Then the portrait cut.

---

## 11. Script (reference)

**Global rules.** Geist Sans captions, Geist Mono terminals. Everything typed; camera follows the cursor, zooms out when done. UI mocked, numbers real with a source line.

**1 · Open.** Rain clip. Title: *Software can finally be for everyone.* Dissolve to paper.

**2 · The problem.** *We spend too much time containing agents, and not enough time letting them do work for us out where work happens.*

**3a · Any harness.** Claude Code, Codex, Cursor, Muse Code side by side; each warns, then exposes a key. *Any harness. Same leak.*

**3b · Add a vault.**
```
$ ai2 vault put --name stripe --approved-url https://api.stripe.com/
Secret: ••••••••••••••••
✓ stripe locked in the vault
```
*Agents get a short-lived pass, never the key.*

**3c · Stat.** *28.65M secrets leaked on public GitHub in 2025.* *70% of those leaked in 2022 were still valid three years on.* (GitGuardian)

**4a · Stat.** *Known exploited vulnerabilities: 245 in 2025, 283 in 2026 so far.* (CISA KEV) *There's no time to check everything by hand.*

**4b · Review.**
```
$ ai2 --run-path ./notes --objective "Add a search box"
Checking the stack against exploits published in the last few days…
```
Security cards. *One command. Four checks. Two scores.*

**5a · Stat.** *One app a day. Thirty folders by the end of the month.* (arithmetic, not a survey) *~1M new projects a week, on Lovable alone.* (Lovable, self-reported) *They get lost in all the folders.*

**5b · Register.**
```
$ ai2 apps register --name Notes --description "Plain notes you can search" --command "deno task start" --openapi /openapi.json --icon favicon.svg
✓ Notes added
```

**5c · Pinned.** App cards, Notes lands first. *Register the ones that matter.*

**End.** Vault · Security · Apps cards. *Don't sandbox agents. Double their stopping power with ai².*

---

## Open questions

- The `✓` output lines are my guess; `docs/commands.md` has the real output and step 3 of the build order matches them.
- The security cards' content is illustrative and labelled "example". If you'd rather show a real review of a real folder, we record one with `ai2 --run-path` on demo data (`docs/demo-data.md`) and type the real output instead.
- "Double their stopping power" is kept as written. If the end line should be the original "Instead of sandboxing them, let's double their stopping power", say so.

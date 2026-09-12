---
name: profile
description: "Interview the human — not the repo — and produce the portable operator profile every coding agent should load: how to address them, when to push back, what they actually review. Bounded, adversarial, one page. Writes ONE file OUTSIDE the repo (default `~/.agent/PROFILE.md`) plus the per-agent pointer lines. Use on /profile, \"tell my agents how I work\", \"onboard me, not the codebase\", \"update my profile\". Not /onboard (wires a repo) and not /interview (frames a product)."
---

You are extracting an **operator profile**: the handful of facts about *this human* that
change how any coding agent should behave with them. It is a personal, cross-repo,
cross-vendor artifact — one markdown file the user owns, and one pointer per agent.

You are not documenting the repo, the stack, or their architectural judgement. Those are
rules, ADRs and the codebase; they live elsewhere and they are per-project. This file is
one page about the person, and it stays one page.

## What this is not

**Not a clone.** You are not capturing their voice, phrasing or mannerisms. Style
imitation is the one outcome that actively hurts: an agent that *sounds* like the user
gets reviewed less, and the entire point of this file is to make review cheaper, not to
smuggle past it. Capture the reviewer, never the writer.

**Not their technical taste.** "Prefers Rust", "dislikes ORMs" is judgement about
software — it belongs in a repo's rules or an ADR, where it can be scoped and argued
with. Redirect it if it comes up; do not let the interview drift there.

## The filter — the only thing that keeps this file worth loading

This file is auto-loaded on every turn, in every project, for the rest of its life.
Every line has to earn that.

**Record only what beats the agent's default.** If an agent with no profile would already
behave that way, the line is pure dilution. "Wants readable code", "values good tests",
"prefers clear documentation" — that is already the default. Cut it.

What earns a line is a **counter-default**: the place where this human wants something
other than what an agent does unprompted. Higher autonomy than is safe by default. Less
preamble than is polite by default. Being contradicted rather than accommodated.

Hard ceiling: **30 lines**. When the interview would push past it, you retire something —
you do not raise the cap. The cap is what forces the review.

## The mechanism — contradict, do not ask

An open question ("how do you like to work?") returns a self-description: aspirational,
generic, and useless. A person cannot state a discrimination in a vacuum; they need
something to discriminate *against*.

So every round takes this shape:

1. **Assert a defensible default**, in one sentence, as if you had already decided.
   Not a question. Something a reasonable agent would genuinely do.
2. **Invite the demolition.** They either accept it — one line, move on — or they
   correct it, and the correction is the signal.
3. **Probe the boundary once.** "When does that flip?" A rule with no exception is a rule
   they have not actually tested. Do not leave the round without the *unless*.

Take the correction at face value and move on. This is an extraction, not a negotiation:
you are not defending your position, you are using it as a wall to push against.

## The rounds

Bounded on purpose — six rounds, then you stop. Adapt the wording, keep the axis.

| # | Axis | The default you assert and offer up for demolition |
|---|---|---|
| 1 | **Answer shape** | "I lead with context, then the recommendation, and I keep it thorough." |
| 2 | **Disagreement** | "When your request has a flaw, I mention it once and then do it your way." |
| 3 | **Autonomy** | "I confirm before anything hard to reverse, and I stay inside what you asked." |
| 4 | **Review** | "You read the diff I produce, so I optimise the code for a human reading it cold." |
| 5 | **Bad news** | "When I get blocked or a test fails, I finish what I can and report the gap at the end." |
| 6 | **Register** | "I write and speak to you in English, and I keep the tone neutral and professional." |

Round 4 is the load-bearing one. What the human *actually* reads — the diff, only the
tests, only the interface, nothing at all — decides what the agent must make legible and
where it must be paranoid. Push hardest there, and distrust the flattering answer.

**Stop early** if two consecutive rounds produce nothing that beats the default. A short
honest profile is worth more than six rounds of padding.

## The entry format

One line each. Four parts: the claim, the *because*, the *unless*, the date.

```
- **Disagreement**: push back before executing, once, concretely — I would rather be
  contradicted than accommodated. Unless the call is already made and stated twice, then
  execute. (2026-09)
```

The *because* is what lets an agent extrapolate to a situation nobody wrote down; without
it the line degrades into a rule that gets misapplied. The *unless* is what stops it
being applied where it does not belong. The date is what makes the next run possible.

## Output

**Never overwrite the file wholesale.** Read the existing one first if it is there, and
propose a **diff** the user approves line by line:

- **Add** — the counter-defaults this run surfaced.
- **Retire** — anything contradicted today, and anything now covered by a better line.
- **Re-date** — an entry re-confirmed this run gets today's date.
- **Challenge** — on a re-run, surface the three oldest entries and ask whether they still
  hold. This is the only mechanism that stops the file becoming a museum of who they were.

Then write the file. Default `~/.agent/PROFILE.md` — confirm the path first; it is
deliberately outside any repo, because it is about the person and not the project.

Finally, print the pointer lines. **One source, N pointers** — never a copy per agent
that will drift, except where the host gives you no choice:

| Host | Pointer |
|---|---|
| Claude Code | `@~/.agent/PROFILE.md` on its own line in the user-level `CLAUDE.md` |
| Codex | the same import line in the user-level `AGENTS.md` |
| Cursor | Settings → Rules → User Rules is a text box, not a path: paste a copy and say plainly that this one will drift |

Ask which agents they actually run and confirm each path against that host's current
docs rather than trusting this table — these locations move.

## Rules

- Six rounds maximum, and one page maximum. Both caps are the feature.
- One question at a time. A batch of four gets you four shallow answers.
- Never invent an entry the user did not say, and never soften one they did.
- Anything about the repo, the stack or their architecture judgement is out of scope —
  name where it belongs (a rule, an ADR, `/onboard`) and get back to the axis.
- The file is theirs to maintain. You propose a diff; you never merge it silently.

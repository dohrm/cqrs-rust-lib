---
name: debrief
description: "Retrospective on the implementation just finished in this conversation — the path taken, the choices made and the alternatives declined, the code's sharp edges worth remembering, what surprised you, and how the prompt that started this could have been clearer. Answers in the chat only, writes nothing. Use on /debrief."
disable-model-invocation: true
---

You just built something. Before the context evaporates, extract what only this session knows — the parts a diff can't show and a commit message won't carry: why you chose the path you took over the alternative, which lines are load-bearing in a way that isn't visible from reading them cold, and what a better prompt would have skipped you straight to.

Answer inline in the chat, in this order. Skip a section only if it is genuinely empty — never pad it.

1. **What was built, and the path taken.** Not a diff summary — the order of discovery: what you tried first, where it broke, why you changed course. If the path was straight, say so in one line.
2. **Choices made, and what was declined.** For each non-obvious decision: what you picked, what the real alternative was, and the reason the alternative lost. Skip decisions with no real alternative.
3. **Sharp edges in the code.** The lines a future reader (human or agent) will misjudge without this context: an invariant enforced two files away, a name that undersells what it does, an order dependency, a TODO left on purpose. Point at `file:line`.
4. **What surprised you, worth keeping.** A pattern, a gotcha in this codebase or toolchain, a wrong assumption you started with — anything worth knowing *before*, not after, hitting it again.
5. **How the prompt could have gotten you there faster.** Name plainly what was missing or ambiguous in the request that cost turns to discover — a constraint stated too late, an assumption you had to guess at, scope that only became clear mid-task. If the prompt was already sufficient, say so — don't invent friction to fill the section.

## Rules

- No file is written. This is a chat answer, not a document — if the user wants it kept, that's memory or a commit message, not this skill's job.
- Never invoked by the model on its own — the user asks for it explicitly, because a debrief after every task would be noise, not signal.
- Speak in specifics — file paths, function names, the actual alternative that lost. "Made good architectural decisions" is not an entry.
- Section 5 is feedback about the request, not a complaint about the user — frame it as what would have helped, not what they did wrong.

---
name: devlog
description: Summarize a day's git commits into a diary-style devlog entry. Provide a date (YYYY-MM-DD) or omit to be asked.
allowed-tools: Bash(git log:*), Bash(mkdir:*), Bash(cd:*), Read, Write, Glob
---

# Devlog Entry Generator

Summarize a day's git commits into a casual, diary-style devlog entry.

## Process

### Step 1: Determine the Date

If the user provided a date argument (in any reasonable format like `2024-03-15`, `March 15`, `3/15/2024`), parse it to `YYYY-MM-DD` format.

If no date was provided, ask the user using `AskUserQuestion` with a free-text prompt for the date they want to summarize.

### Step 2: Fetch Commits from Both Repos

Fetch commits from **both** the product repo and the private strategy repo:

**Product repo** (current directory):
```bash
git log --after="YYYY-MM-DDT00:00:00" --before="YYYY-MM-DDT23:59:59" --format="%H %ai %s" --stat
```

**Strategy repo** (`~/code/immerse-yourself-strategy/`):
```bash
cd ~/code/immerse-yourself-strategy && git log --after="YYYY-MM-DDT00:00:00" --before="YYYY-MM-DDT23:59:59" --format="%H %ai %s" --stat
```

Replace `YYYY-MM-DD` with the target date.

### Step 3: Handle No Commits

If BOTH repos have empty git log output, tell the user there were no commits on that date and stop. If only one repo has commits, proceed with what's available.

### Step 4: Write the Devlog Entry

1. Create the devlog directory if it doesn't exist: `mkdir -p devlog`
2. Compose a short, casual diary-style **paragraph** (not bullet points) summarizing what was accomplished that day. Guidelines:
   - Use **past tense, first person** ("I added...", "I fixed...", "I spent time on...")
   - Keep it conversational and natural, like a personal dev diary
   - Mention the key themes/areas of work without listing every single commit
   - 3-8 sentences is ideal
   - Include a date header line at the top: `# YYYY-MM-DD`
   - Add a blank line after the header, then the paragraph
3. Write the entry to `devlog/YYYY_MM_DD_devlog.txt` (underscores in filename, hyphens in header)

### Strategy Repo Content Rules

When weaving in strategy repo activity, **keep it high-level and public-safe**:

- **DO mention**: general themes like "business planning", "market research", "license auditing", "competitive analysis", "roadmap planning", "issue triage"
- **DO NOT mention**: specific competitor names, pricing numbers, revenue projections, financial details, or strategic decisions that would reveal competitive intelligence
- **DO NOT mention**: the strategy repo by name or URL — just reference "planning" or "strategy work"

**Good**: "I also spent time on commercialization planning — auditing sound licenses, defining what goes in the free tier vs premium, and researching the legal landscape for redistributing open-source audio."

**Bad**: "I found that Syrinscape charges $10.99/month and has 80% market share, so I'm pricing our packs at $9.99-$14.99 to undercut them."

### Example Output

```
# 2024-03-15

I spent most of the day working on the lighting engine. I refactored the bulb group initialization to be async and added fire-and-forget patterns for sending commands, which made the animations feel way more responsive. I also fixed a bug where the daemon would crash when a bulb went offline mid-animation. On the business side, I did some planning around content licensing and premium pack structure — nothing to announce yet but the direction is getting clearer. Toward the end of the day I added a new "Wizard's Tower" environment with some purple/blue cycling lights that turned out pretty cool.
```

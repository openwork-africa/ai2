# Outputs

`report.html` is a standalone page: one card per rating with its factors, then the stack and its share of the risk, recent exploits, findings, per-item scores and notes. It follows the system's light or dark theme.

`review.json`:

| Field | |
| --- | --- |
| `version`, `folder`, `objective`, `generated_at`, `harness` | the run |
| `ratings` | one entry per step, in order: `key`, `name`, `short`, `riskier` (true when a higher score is more risk), `rating` (`score` and `factors`, each `name`, `score`, `why`) or null with `missing` saying why, and `items` when the step scores items one by one (`index`, `text`, `score`, `factors`) |
| `technologies` | `name`, `kind` (`language`, `library`, `os`), `risk`: its percent of the risk |
| `exploits` | `id`, `technology`, `severity`, `summary`, `fixed_in` |
| `findings` | `title`, `technology`, `file`, `severity`, `likelihood`, `impact`, `confidence`, `fix`, `status` (`fixed` or `open`) |
| `spec` | `os`, `languages` (`name`, `lines`, `share`), `libraries` (`name`, `version`, `ecosystem`) |
| `notes` | what could not be done |

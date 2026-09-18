<!-- Use a conventional title, such as fix(core): preserve selection on undo.
     Describe the problem, resulting behavior, and evidence. Remove this comment. -->

## Change

Describe the trigger and what changes for the caller. Include migration notes for
API, Cargo feature, or behavior changes.

## Validation

List the checks run and their results. Include captured frames for rendering
changes and reproducible measurements for performance claims.

## Checklist

- [ ] `./x check` passes; relevant PTY and documentation checks pass
- [ ] Bug fixes include regression tests that fail on the original defect
- [ ] Affected adapters, examples, comments, and guides are updated
- [ ] User-visible changes and migration steps are described for the release notes
- [ ] Security-boundary or dependency changes are explained
- [ ] I reviewed the complete change and can explain and maintain it, including any AI-assisted work

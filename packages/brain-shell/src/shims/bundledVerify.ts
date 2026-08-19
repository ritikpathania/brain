import { parseFrontmatter } from '../../vendor/claude/utils/frontmatterParser.js';
import { registerBundledSkill } from '../../vendor/claude/skills/bundledSkills.js';

export const SKILL_MD = `---
name: verify
description: Verify that a code change actually does what it's supposed to by exercising it end-to-end and observing behavior — drive the affected flow, not just tests or typecheck. Run before committing nontrivial changes; bootstraps this repo's project verify skill if none exists yet. Don't invoke it on a diff that only touches tests, docs, or other code with no runtime surface to drive (a change to product source always has one) — there's nothing to observe.
---

**Verification is runtime observation.** You build the app, run it,
drive it to where the changed code executes, and capture what you
see. That capture is your evidence. Nothing else is.

**Don't run tests. Don't typecheck.** Running them here proves you
can run CI — not that the change works. Not as a warm-up,
not "just to be sure". Tests are CI's job. Typechecking is CI's
job. Your job right now is proving the runtime surface actually
moves the needle.
`;

const { frontmatter, content: SKILL_BODY } = parseFrontmatter(SKILL_MD);

const DESCRIPTION =
  typeof frontmatter.description === 'string'
    ? frontmatter.description
    : 'Verify that a code change actually does what it\'s supposed to by exercising it end-to-end and observing behavior — drive the affected flow, not just tests or typecheck. Run before committing nontrivial changes; bootstraps this repo\'s project verify skill if none exists yet. Don\'t invoke it on a diff that only touches tests, docs, or other code with no runtime surface to drive (a change to product source always has one) — there\'s nothing to observe.';

import { registerArtifactDiagrammingSkill } from './bundledArtifactDiagramming.js';

export function registerVerifySkill(): void {
  registerArtifactDiagrammingSkill();
  registerBundledSkill({
    name: 'verify',
    description: DESCRIPTION,
    userInvocable: true,
    files: {},
    async getPromptForCommand(args) {
      const parts: string[] = [SKILL_BODY.trimStart()];
      if (args) {
        parts.push(`## User Request\n\n${args}`);
      }
      return [{ type: 'text', text: parts.join('\n\n') }];
    },
  });
}

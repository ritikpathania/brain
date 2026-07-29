# Executive Summary

Innernet (innernet.live) is an early‐stage startup offering a **“versioned AI memory platform”** that persistently records a user’s projects, ideas and context across multiple AI tools. Its mission is to externalize personal and project memory so it can be reused (“the internet, but yours”) rather than re-explained in every chat.  The core product is a **web app + API (Model Context Protocol endpoint)** that connects once to any AI tool (ChatGPT, Claude, Cursor, etc.), so that all tools share the same context “map” of the user’s projects and preferences.  Innernet emphasizes strong security and privacy: it enforces **row-level data security, hashed tokens, opt-in encryption, GDPR rights, and no tracking cookies**. 

The platform appears targeted at solo developers, creators, and researchers using AI extensively – essentially anyone who suffers “context loss” between AI sessions. It positions itself as a **foundational memory layer** for AI tools, claiming to be “like a searchable, evolving second brain”. As of mid-2026 Innernet is in early beta: it has launched on Product Hunt, has a functioning dashboard and API, but **no published pricing** yet.  The company (founded ~May 2026 by Sahil Verma and Shaurya Dhand) is likely self-funded/pre-seed (no press on funding), with a small team building out features (e.g. “daily reflection” letters, memory scans) in rapid iterations. 

**Key findings:** Innernet’s strength lies in its clear mission, integrated feature set (context maps, API, Git-like versioning) and privacy-first stance. Its weaknesses include very early stage (unproven market fit, no pricing or wide adoption yet) and reliance on network effects of AI tool adoption. Opportunities include a growing demand for persistent AI context in workplaces and developer tools, while threats include competition from established note-taking/AI apps (Mem.ai, Notion AI, Personal.ai, etc.) and potential security or IP risks.  

**Recommendation:** Further due diligence is advised before adoption. Potential adopters should review data-handling and security controls, monitor platform maturity, and consider privacy implications. Pilot testing Innernet with non-sensitive data could evaluate its utility, while mitigation steps (like strong encryption and data export procedures) should be prepared in case of platform failure or exit.

## Site Purpose / Mission

Innernet markets itself as **“an AI memory platform”** – essentially a **universal memory layer** that “saves and carries your context no matter where you go or what app you use”. The purpose is to **eliminate repeated explanations** by storing project outlines, decisions, voice, and preferences as a versioned memory, readable by any connected AI.  For example, the site’s FAQ defines it as “a memory that keeps itself” by “watching the work you do across your AI tools” and handing it back to the next session. The company narrative (e.g. its blog) underscores a broader mission of **preserving human memories outside our imperfect brains**.  

Citing the homepage and blog: “innernet — the internet, but yours. A living, versioned memory for your ideas and your code, read by every AI you connect”. The founders describe it as a way for users to have “a palace for your mind & memories, arranged the way you see best”. Innernet emphasizes privacy and personalization: a user’s data “answers only to you” via database-enforced row security, and the AI tools see a unified view of the user’s projects (dimensions and nodes) whenever they connect. The tagline “any AI, any tool — always current, always yours” encapsulates the mission to serve as a **constant contextual backbone** across AI workflows.

## Products / Services Offered

Innernet’s product is a **cloud platform** comprising:
- **Core Memory Service (MCP API)**: A single endpoint (innernet.live/api/mcp) implementing the open *Model Context Protocol*. Users connect their AI tools (ChatGPT, Claude, Cursor, etc.) to this endpoint so that all tools read/write the same persistent context. No keys need to be manually copied; connecting once via OAuth 2.1 grants the AI access to the user’s memory.
- **Web Dashboard / Context Maps**: A web-based UI where users can view “context maps” (projects with dimensions and version history, like a code repo) and the evolving memory facts. The interface features project lists, timelines, and “orbit view” of tasks as seen in screenshots. It also includes a conversational “agent” called *Netti* that proposes memory insights (e.g. summarizing how the user’s voice and plans evolve).
- **Memory Agent & Insights**: A server-side agent consolidates captured conversations into facts, suggests updates (e.g. “two new stakeholders noticed”), and generates daily reflection letters or trend analyses as part of the experience (noted in blog updates). 
- **REST API / Developer Tools**: In addition to MCP, Innernet offers a standard REST API for programmatic access to projects and commits (see “Developers” docs). Users can mint API keys from the dashboard and use commands (curl examples) to list and manipulate projects.  
- **Integrations**: Built-in support for any MCP-aware tool, plus the option to write an *anchor file* linking any Git repo or local notes into the context map (ScoutForge review mentions a “Git-like anchor for code repos”). They list third-party sub-processors (Supabase, Vercel, DeepInfra, etc.) for various backend services.

No separate product lines or tiers are advertised yet – Innernet is a single **“memory platform”** for AI contexts. As of now it is in beta / early access. The product table below summarizes this:

| **Product / Service**             | **Key Features**                                                                                                      | **Pricing / Access**            | **URL**                                 |
|----------------------------------|-----------------------------------------------------------------------------------------------------------------------|---------------------------------|-----------------------------------------|
| **Innernet AI Memory Platform**  | Versioned context maps, cross-tool memory, personal “second brain”; integrated with ChatGPT, Claude, etc.             | Beta (no public pricing; free/beta access)  | innernet.live (homepage)               |
| **MCP Endpoint (innernet.live/api/mcp)** | Connect once to any MCP-compliant AI tool; auto-handles OAuth and token storage; context auto-sync                      | Included in platform            | innernet.live/api/mcp (endpoint)        |
| **Developer API (REST)**         | Manage projects/dimensions/commits programmatically; mint API keys; use curl/SDK to integrate memory into apps         | Included in platform            | innernet.live/developers               |
| **Web Dashboard / UI**           | View projects as “context maps”; Git-like commits/branches history; timeline/grid/list views; customize memory (Netti) | Beta (login required)           | innernet.live/dashboard (sign-in)      |

_Source: Innernet documentation and site content._ 

## Target Audience & Market Positioning

Innernet is positioned as a **productivity/AI tools enabler** for tech-savvy users. ScoutForge’s profile explicitly identifies the target demographics: **“solo developers and indie hackers, content creators and writers, researchers and analysts”** who need persistent memory across AI sessions. In other words, anyone building software or content with AI assistants who faces “losing context between sessions” or repeated onboarding. The value proposition is strongest for **knowledge-workers integrating many AI tools**: the memory stays with them, so ideas, research, brand voice, or code context doesn’t get fragmented. 

Market-wise, Innernet sits in the emerging “AI memory layer” niche. Unlike general note-taking apps, Innernet’s pitch is **live memory**: it listens in real time to AI conversations and automates context capture. This places it between personal knowledge managers (Mem.ai, Notion AI) and AI assistants (personal bots like Personal.ai). Its differentiation is deep AI-tool integration and fine-grained security. Early marketing (Product Hunt launch, social media posts by founders) stresses cutting-edge appeal: e.g. one description calls it “a persistent, versioned memory layer for your digital life”. 

No official “go-to-market” or pricing model is visible yet. The site requires users to *sign up* (via Google/Github/email magic link), implying a freemium or invite-based launch. The Reddit thread shows the founder actively sharing in local tech forums (“launching … innernet.live”). The “YouTube, blogs, and posts” angle is likely important; however, as of mid-2026 Innernet lacks enterprise branding and is clearly in startup mode. Its positioning targets early adopters and innovators eager to try novel AI productivity tools.

## Pricing & Monetization

No formal pricing or monetization scheme is public yet. The website and docs mention **no plans or fees**. Indeed, both the interactive UI timeline and ScoutForge note that *pricing is “shipped after onboarding”*. This suggests Innernet is currently **beta/free**, with future pricing TBD. All technical docs describe usage but not cost. The Reddit conversation shows one user asking about pricing and the founder replying “using MCP & REST API” (not addressing cost), implying the focus is still on product, not pricing.

Given standard SaaS practice and ScoutForge’s comment (“unproven, early beta… sits ahead of basic note-taking alternatives but behind established players”), the business model is likely to be **subscription-based for power users or enterprise teams**. Potential models include per-seat or per-usage fees, plus possibly premium features (e.g. extended storage, encryption). Innernet’s Terms of Service (updated May 19, 2026) confirms “during early access… our total liability is limited to fees you’ve paid (which is zero),” implying paid plans are expected later.

In summary, **current monetization** appears none (free beta). **Future plans** are unclear, but Innernet will probably charge for continued use once out of beta, as hinted by the active discussion of when/how to introduce pricing. The ScoutForge review notes this gap: “being in early beta with no pricing, demos, or user feedback… remains unproven”.

## Feature Set & UX (Desktop/Web)

Innernet is a **web-based application** (hosted globally via Vercel) and is likely accessible on any device via browser. There is no mention of a mobile app; instead, users interact through:
- **Dashboard interface**: The site UI shows interactive elements (“projects”, “tasks”, “to do/done lists”) with layouts (grid, list, timeline views). Screenshots and site text hint at a polished React-like interface that organizes context maps into sections (“you”, “projects”, “tasks”, “parked”). AI chat sessions can be initiated from the site, capturing content into the memory.
- **AI Tool Integration**: Rather than a standalone app, much of the UX is embedded within AI tools. For example, as shown on the homepage mockup, a user can type “Save this conversation to a new innernet project” in ChatGPT and the memory agent does the rest. So the primary interaction may be **in-context within Chat/Code tools**, with the web UI for review. 
- **User Experience**: Innernet’s tone is friendly and personal (“warm, plainspoken, no jargon” is explicitly referenced for voice). The custom avatar Netti (Innernet’s memory AI assistant) suggests a personable UX. The product encourages low friction: “You keep using your tools exactly as you do, and the memory accumulates on its own”. 
- **UX Caveats**: Some elements are unfinished (“disclosure classes coming”, “session tooltips not shown”), indicating a work-in-progress UI. Also, the site sets a custom favicon and a distinctive aesthetic (green/purple) but details on mobile responsiveness are unknown. The dashboard requires login (OAuth magic link or social login).

Overall, the feature set is rich: versioning (commits, branches on context maps), AI-driven insights (auto-tagging preferences, moodboards), and integration breadth. The downside is current UX complexity (project/dimension concept is advanced) and dependency on AI tools to use effectively. There is no official mention of a dedicated desktop or mobile app beyond the web UI and browser-based sign-in.

## Technical Stack & Hosting

Innernet’s stack can be deduced from its privacy policy and sub-processor list, as well as job postings. Key components: 
- **Frontend/Hosting**: Vercel (global CDN) hosts the web UI. The use of React/Next.js is implied by client-side interactivity and modern site design (Reddit comments mention “Cloudflare Pages, Astro” but actual hosting is Vercel). The mention of “no tracking cookies” and use of PostHog suggests a standard JavaScript frontend.
- **Backend/Database**: Supabase is listed as the database and auth provider. Thus the memory data is likely in a PostgreSQL database with Supabase’s access control. Row-level security is a Postgres feature, aligning with Innernet’s description.
- **APIs**: Innernet provides both an MCP server endpoint and a REST API. It likely runs on Node.js/TypeScript (common for Supabase and Vercel stacks). OAuth 2.1 indicates standard auth flows with providers (Google, GitHub).
- **Machine Learning Components**: The memory agent uses inference processors (“DeepInfra” and “Fireworks” running “DeepSeek” models). These are likely custom or third-party LLM inference services (DeepInfra, Fireworks) used to summarize or classify content before storing. OpenAI/Anthropic are only invoked *when the user connects Claude* – implying Claude is used for both context (client side) and also possibly on server for agent reasoning.
- **Analytics/Monitoring**: They use Google Analytics (in cookieless mode) and PostHog (self-hosted via innernet.live proxy) to collect aggregate usage data. A public status page polls service health every 30s. 
- **Security**: The site uses hashed credentials, OAuth, column encryption options, and publishes a security policy on GitHub. The TLS endpoint (innernet.live) and hashed tokens suggest industry-standard protections. They also mention not storing passwords at all.
- **Hosting/Region**: Supabase in US East; Vercel global edge for the front-end; email via Resend. This indicates cloud-native infrastructure likely on AWS/GCP via these services. 

In summary, Innernet is built on a modern serverless architecture (Vercel + Supabase) with proprietary AI inference integrated. The tech stack is not explicitly disclosed but is consistent with Node/React/AWS (via Vercel/Supabase).  

## Security, Privacy, and Compliance

Innernet advertises **strong security and privacy safeguards**. According to its Security page, **“every row answers only to its owner, enforced in the database”**. Access tokens are hashed at rest, and there is optional per-project encryption (user-supplied passphrase) for sensitive fields. Privileged actions are audited (90-day log visible only to the user). The privacy policy states that they keep **no advertising or cross-site tracking cookies** (Google Analytics is cookieless; PostHog uses local storage). GDPR compliance is explicitly honored: users can export or delete all their data on demand, with immediate effect and 30-day purges.

Innernet’s handling of user content is also outlined: **users “own what they save”** and only grant Innernet a narrow license to store/process it for service delivery. They explicitly forbid uploading illegal or copyrighted content, or anything violating upstream AI providers’ terms. Crucially, any AI client connected will see the memory (subject to that provider’s privacy rules). The site warns “once an AI you connect reads your memory, it’s in that AI’s hands”.  Data is *not* sold, rented, or used to train AI models – the privacy policy guarantees that user memory is never used for training.  (Innernet does use user data internally only for service features, via private inference subprocessors.)

**Trackers:** There are no ads or third-party trackers beyond analytics (GA & PostHog). The use of GitHub and Supabase implies code transparency for security (they link to a public security policy repo). 

In summary, Innernet appears **privacy-focused**: row-level DB isolation, hashed credentials, no hidden tracking, and compliance with data rights. That said, it does rely on external services (Supabase, Google, Anthropic) for infrastructure, meaning data is ultimately stored in US region and might be subject to local laws. The Terms of Service are permissive (“as-is, no warranty; no uptime guarantee during beta”), reflecting the early stage.  Overall, while early audits are not available, the published policies are thorough and privacy-friendly, which is a strength in this space.

## Business Model, Funding & Team

Innernet was **founded in May 2026** (per LinkedIn profiles of founders) by Sahil Verma and Shaurya Dhand. Both have backgrounds in tech/design (LinkedIn indicates prior roles at design/tech firms). The founding team seems small (blog mentions “we are a small team”) and focused on product development. There is no public information on outside funding (no Crunchbase entry or press release). It is likely bootstrapped or seed-funded quietly. The absence of funding announcements and the very early stage suggest it’s not VC-backed yet (ScoutForge calls it “early beta”). 

The company structure appears lean; team members are likely based in India (founders are in India, and a local Ludhiana developer posted the launch). The Reddit announcement said “built in Ludhiana”. There is no separate corporate website – the “Company” section of innernet.live mostly links to blog updates. They encourage contact via support email (privacy/security issues to yours@innernet.live) but no clear investor or advisory board listings. 

Business model: presumably software-as-a-service. Long-term revenue may come from subscriptions (per-seat or usage), or enterprise licensing. The Terms of Service clause limiting liability to “fees paid” implies a paywall is planned. There is no obvious alternate revenue: no ads, no data monetization (explicitly prohibited), so subscription is likely. However, current beta users pay nothing. 

Team (beyond founders) is not disclosed, but given the rapid development (product hunt launch, weekly blog) they may have a few additional engineers or advisors. The site does not list jobs or org structure, so team size and roles are unknown. 

## Competitors & Market Landscape

Innernet’s concept overlaps with several categories: **personal knowledge management (PKM), AI assistants, and AI productivity tools**. Direct analogues include:
- **Mem.ai** – an AI-driven note-taking app that auto-organizes personal knowledge. However, Mem is more about search/notes than live multi-tool memory.
- **Notion AI** – integrated AI in a popular workspace; it stores project context but requires manual management (Notion is collaborative but not automatically syncing across all AI chats).
- **Rewind.ai** – a personal memory recorder (screen and audio); local and not focused on multi-tool integration.
- **Personal.ai** – builds a chat-based personal memory/assistant, similar goal of extending human memory but tied to one app.
- **Other AI context layers**: emerging startups like MemVerge (memory infrastructure) or Mem0 (open-source memory agents) target enterprise/AI memory infrastructure rather than personal use. Their focus (LLM internals) differs.
- **Generic alternatives**: Traditional PKMs (Evernote, Roam) or project management (Jira, Confluence) partially solve context retention but lack real-time AI integration. 

According to an independent review, Innernet’s unique **edge is its live synchronization with any AI via MCP and built-in versioning**. That said, the competition is strong: major AI platforms could incorporate similar memory features (e.g. Microsoft, Google). Also, if users are in locked ecosystems (e.g. only ChatGPT), they may not adopt a neutral layer.  The ScoutForge review notes that while Innernet “sits ahead of basic note-taking alternatives in AI-tool depth”, it is “behind established players in maturity and ecosystem”. Thus, the competitive threat is high: any successful integration by big AI vendors (like a persistent memory in ChatGPT) could diminish the need for Innernet.

In summary, the market is **nascent but growing**. Innernet is not alone, but its specific focus on a unified memory layer is relatively unique. Alternatives range from simple note tools to ambitious personal AI companies. The competitive landscape is unsettled, and Innernet will need to accelerate development to solidify its niche.

## Reviews, Social Media & Community Feedback

**Third-Party Reviews:** Independent coverage is limited. A July 2026 review on ScoutForge (a tech app review blog) gave Innernet a 63/100 score. That review praises the concept and features (“compelling vision”, strong security details, seamless integrations) but notes the lack of live demos or pricing and the uncertainty of product-market fit. No mainstream tech press articles are found. Innernet’s own marketing (blog posts by founders) serves as its primary narrative.

**Social Media / Community:** Innernet has an active presence on social platforms, primarily LinkedIn and Instagram, posting mission statements and updates. For example, co-founder Shaurya Dhand’s LinkedIn post echoes the tagline that “every AI tool forgets you… innernet doesn’t”. On Reddit, the product has drawn some attention: in an r/ludhiana thread the founder announced the launch and engaged with users on technical questions. Comments there were supportive (“really good idea”) and users showed interest in contributing. 

No user testimonials or critiques are publicly available yet (ScoutForge noted “0 Feedbacks”). There’s no clear sentiment data (Twitter/X scraping was inconclusive), but the founder’s interactive posts and early adopter questions indicate a **positive, curious sentiment** from the developer/AI community. The absence of negative press suggests the project is under the radar.

## SEO, Traffic Estimates, Analytics Signals

No public traffic metrics are available. As a brand-new site (launched mid-2026) Innernet.live likely has **minimal web traffic** beyond initial launch spikes. The absence of a public analytics report suggests they rely on internal Google Analytics (cookieless) and PostHog to track usage. 

SEO-wise, Innernet’s content is fairly specialized. The site includes relevant keywords (“AI memory platform”, “context map”, “Model Context Protocol”), so it may rank for niche searches. However, without backlinks or high domain authority, organic traffic is likely low. There is no listing in Alexa/SimilarWeb rankings known, which implies that traffic is not large enough to register. We could not find any third-party estimate of visitor count or pageviews. 

Given the target audience is technical (developers/influencers), traditional SEO may be less relevant than community outreach. Analytics signals like number of signups, API calls, or Product Hunt upvotes (not published) would be better indicators, but these are not public. In short, **visibility is limited**: Innernet relies on word-of-mouth, social media, and targeted outreach. Any future SEO strategy would need to include content marketing (blogs on memory/AI topics) and developer community engagement.

## Legal Risks or Red Flags

Review of Innernet’s policies and industry context yields the following legal considerations:

- **Intellectual Property:** Users upload their own data (notes, code, ideas) into Innernet. The Terms affirm user ownership but grant Innernet a license to store/process it. There is a risk if users store third-party copyrighted content; the Terms explicitly forbid storing infringing material. However, policing this is tricky; Innernet would likely be protected by DMCA-like safe harbor if it promptly removes infringement notices. Companies should beware of storing any sensitive corporate IP there, since liability for leaked IP (e.g. if an AI model derivative was inadvertently stored) is not spelled out.
- **Privacy & Data Protection:** Innernet’s privacy policy is robust, but there is the usual risk of data breach. The platform claims encryption and is not sharing data, but any cloud service can be hacked. If a user’s private project were leaked, responsibility could become a legal issue. The GDPR compliance commitments (export, delete) minimize regulatory risk, but companies should confirm they meet internal data retention requirements.
- **Terms of Use:** The ToS limit liability and disclaim warranties. As an early beta, Innernet does not guarantee uptime or data portability (“schema is still evolving”). Customers need to be aware of this risk: in practice, they should treat Innernet as experimental and avoid mission-critical use until matured. 
- **Security Standards:** While Innernet claims strong security (see above), it’s not certified (no mention of SOC2, ISO, etc.). For enterprise use, auditors may want evidence (penetration tests, etc.). Lack of formal security certification could be a risk. 
- **Dependency on Third Parties:** Innernet uses Supabase, Google Analytics, and Anthropic (for Claude). Legal issues related to those services flow through (e.g. Supabase’s data storage terms). The Terms mention not violating “upstream providers” policies, which is unusual. It suggests if a user connects OpenAI or Anthropic illegally, Innernet disavows liability. This could create gray areas about data jurisdiction (Anthropic’s servers) or if an AI provider disallows certain data.
- **Trademarks/Names:** “Innernet” is a common wordplay (not owned by big corp as far as known). No obvious trademark conflicts. However, careful monitoring of name usage is prudent. The whimsical name has no known red flags.

Overall, no **fatal red flags** emerge in the legal review. The biggest caveat is that Innernet is clearly early-stage and “use at your own risk” – Terms heavily favor the company (limited liability, evolving schema). For any serious deployment, companies should probably run a pilot, obtain data encryption keys, and have an export/backup plan.  

## SWOT Analysis

- **Strengths:**  
  - *Innovative Core Concept:* First-mover advantage as a universal AI memory layer; integrates with many tools via open MCP.  
  - *Security/Privacy Focus:* Strong technical measures (row-level security, encryption, hashed tokens) and explicit no-tracking policy.  Good for sensitive use-cases.  
  - *Ease of Use:* Once set up, memory “auto-accumulates” without extra user effort. Good UX design (warm, jargon-free voice) aims to make it approachable.  
  - *Developer-Friendly:* Git-like features and REST API appeal to technical users and enterprise integrations.  Open protocols (MCP) ensure extensibility.  

- **Weaknesses:**  
  - *Very Early Stage:* Still beta; lacks maturity, documentation, and public usage stats. No proven track record or community yet (0 reviews on ScoutForge).  
  - *Unclear Pricing/Monetization:* No published pricing or revenue; risk of unclear business model for scaling.  
  - *Limited Audience:* Primarily tech-savvy developers/researchers. Non-technical users (older execs, general consumers) unlikely to adopt.  
  - *Dependency on Others:* Heavily reliant on external AI tools and protocols (if MCP support changes, service impacted). If main AI platforms disregard MCP, Innernet loses connectivity.  
  - *Potential Feature Gaps:* Being new, it may lack robust analytics, team features, or offline access that more mature products have.  

- **Opportunities:**  
  - *Growing AI Adoption:* As businesses integrate more AI (GPT-4, Claude, etc.), the need to maintain context grows. Innernet could become the default layer for large teams using multiple LLMs.  
  - *B2B Partnerships:* Possible partnerships with AI platforms, CRM/Education tools, or dev platforms to embed Innernet as the memory backend.  
  - *Customization/Enterprise Editions:* Build specialized versions (on-prem, data residency options) for privacy-critical sectors (healthcare, legal, R&D).  
  - *AI Insights Services:* Leverage the memory agent to offer analytics on user behavior or knowledge gaps, a value-add for enterprises.  

- **Threats:**  
  - *Competition from Tech Giants:* If OpenAI, Google, or Microsoft decide to incorporate persistent memory (e.g. “profiles” or “workspaces”), Innernet’s niche could shrink.  
  - *Regulatory Scrutiny:* Evolving AI regulations (e.g. about personal data in AI models) could complicate how memory services operate, especially across borders.  
  - *Data Breach Risk:* Any security failure would undermine the product’s core trust promise. Even with strong security, breaches can happen.  
  - *User Reliance Risk:* If critical projects are built on Innernet and it fails or shuts down, customers could face major losses. This puts liability pressure on Innernet to maintain service.  

## Recommendations & Next Steps

1. **Pilot Internally:** If considering Innernet, start with a **limited pilot** using non-sensitive projects. Test connecting several AI tools to the same project and evaluate how well context is synchronized. Verify that you can export data via the API in practice.  
2. **Security Validation:** Review Innernet’s security claims. Ensure encryption keys are managed by the enterprise (the passphrase model allows that). Consider having an external pen-test or code review if using it beyond POC.  
3. **Monitor Development:** Keep an eye on Innernet’s roadmap (e.g. blog and GitHub). Key features to watch: published pricing model, SOC2 or other compliance steps, additional AI integrations (new tools).  
4. **Data Governance Plan:** Implement a governance policy. Decide what kinds of data are safe to store (avoid mission-critical secrets or regulated data until further vetting). Use the opt-in encryption for any sensitive dimensions. Document export/deletion procedures.  
5. **Legal Review:** Have legal assess the Terms of Service around liability and content licensing. If Innernet is to be adopted, negotiate an enterprise agreement if possible that addresses uptime and support.  
6. **Competitive Analysis:** Evaluate alternatives (Mem.ai, Notion AI, etc.) to see if they can meet memory needs. Innernet is unique but not the only solution for knowledge persistence.  
7. **Risk Mitigation:** As a risk buffer, maintain local backups of important knowledge outside Innernet (the REST API can be scripted to sync data to local storage or an internal wiki).  
8. **Community Engagement:** Engage with Innernet’s community (Product Hunt, Discord, Reddit). Early adopters often share tips. This also shows demand (e.g. reddit user wanted to contribute to the project).  

**Key Assumptions/Unknowns:** Innernet’s viability depends on continued support of the MCP protocol by major AI providers. The cost structure (when introduced) will affect adoption. We assume the startup remains independent; acquisition by a larger tech company (with different terms) could significantly change the landscape. 

**Conclusion:** Innernet offers a compelling solution to a real pain point, with robust technical design. It is recommended for **experimentation and pilot trials**, especially within tech teams that already rely on multiple AI tools. However, due to its nascent status, one should proceed cautiously: treat it as a promising new tool, not as established infrastructure. Monitor its evolution, and plan mitigations (data backups, legal safeguards) before fully committing to it. 

| Evidence Source                                            | Link / Reference                       | Credibility Note                      |
|------------------------------------------------------------|----------------------------------------|---------------------------------------|
| Innernet official site – Homepage (“AI memory layer”)       | [innernet.live](https://innernet.live/)        | Primary source; official company site |
| Innernet “Getting Started” documentation                    | [innernet.live/docs/getting-started](https://innernet.live/docs/getting-started) | Primary docs; outlines workflow      |
| Innernet Privacy Policy (Jul 2026)                         | [innernet.live/privacy](https://innernet.live/privacy)     | Official policy; details security and stack|
| Innernet Security page                                     | [innernet.live/security](https://innernet.live/security)   | Official info; details security model |
| Innernet Terms of Service (May 2026)                      | [innernet.live/terms](https://innernet.live/terms)      | Official terms; user rights/licensing |
| Innernet blog (“This week at innernet”, Jul 26 2026)       | [innernet.live/company/this-week-at-innernet](https://innernet.live/company/this-week-at-innernet) | Company blog (internal news)          |
| ScoutForge review (“innernet Review”, Jul 10 2026)         | [scoutforge.net/apps/innernet](https://scoutforge.net/apps/innernet) | Tech review site; independent analysis |
| ScoutForge review – “Alternatives” section                 | (same page as above) | Lists competitors; analyst perspective |
| I Love Ludhiana – Business directory listing (“innernet”)  | [iloveludhiana.com/en/listings/innernet](https://iloveludhiana.com/en/listings/innernet) | User-submitted directory listing (India) |
| Reddit r/ludhiana – HackerHouse discussion (Jul 2026)      | [reddit.com/r/ludhiana/...](https://www.reddit.com/r/ludhiana/comments/1up9fsg/any_hackerhouse_in_ludhiana_founders_or_developers) | Community forum; founder announcement & Q&A |
| Innernet LinkedIn post by founder (concept statement)       | (LinkedIn, not public)       | Social media – tagline confirmation  |

**Table: Innernet Products/Features/Pricing (summarized)**

| Product / Service           | Features / Description                                                                 | Pricing / Monetization                   | URL / Access                      |
|-----------------------------|----------------------------------------------------------------------------------------|------------------------------------------|-----------------------------------|
| Innernet Memory Platform    | Versioned context maps (projects/dimensions), cross-tool context syncing, personal voice/mood tracking, daily “reflection” insights. Integrates with any MCP AI (ChatGPT, Claude, etc.), and supports REST API. Built-in security (row-level, encryption).  | Beta (no published price). Plans TBA; likely future subscription. | innernet.live (web signup)       |
| MCP API Endpoint            | Single endpoint (innernet.live/api/mcp) that AI clients use. Handles OAuth2.1 login once and then delivers shared memory to all tools. Convenience: paste URL, sign in, done. | Included in core platform.             | innernet.live/api/mcp (endpoint) |
| REST API (Developer API)    | Enables programmatic project management: create/load projects, read commit logs, issue keys. Example: `curl https://innernet.live/api/v1/projects`. Suitable for CLI tools or integrations. | Included. Potential rate limits? Not specified. | innernet.live/developers         |
| Web Dashboard (UI)          | User interface to view projects (“Context Maps”), timeline, tasks, and memory “orbit”. Netti AI interface for annotations. Responsive web app (Vercel-hosted). | Free in beta; login required.         | innernet.live/dashboard (sign-in)  |

*URLs and features from official Innernet site and docs; pricing noted as “TBA” per current lack of public info.*
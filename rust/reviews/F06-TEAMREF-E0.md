# F06-TEAMREF-E0: frozen Deno team-reference evidence

This is evidence for the planned Rust team-reference resolver. It does not implement a Rust resolver, command route, or member renderer. The separate 48-case corpus at `rust/parity/runner/f06-teamref-frozen-cases/` invokes the frozen Deno `team members` route. An empty `GetTeamMembers` connection makes its `teamKey` request variable the public proof of which team won resolution. The pinned schema validates every fixture and the confined runner compares exact request operations, variables, identity, sequence, stdout, stderr, exit status, and file effects.

The frozen reference is revision `d4fe6fa7358f018fd1da0c6b96ec2b022247e898`, Deno 2.7.9, compiled binary SHA-256 `a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835`, schema SHA-256 `eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a`. The source hashes below pin exact frozen line citations for F06-A/B:

| Frozen source                       | SHA-256                                                            |
| ----------------------------------- | ------------------------------------------------------------------ |
| `src/utils/linear.ts`               | `57289613743ee08facba18e688d5ef639abb81f8992d250523196ec91f535d0e` |
| `src/utils/linear-url.ts`           | `a64eb8e7ecbfe05b5bf030b817df6d89cd060b145bfdcfcf255a6e47b50cb5f5` |
| `src/commands/team/team-members.ts` | `7782e81fcb5f44aaeafcac8bf8b5196e8823de1ab2263018ca6cea584a1434b5` |
| `src/utils/errors.ts`               | `091c83791b9b20dfac3f73e94f4622e1781703b592803afc80a844eaca30e16a` |
| `src/config.ts`                     | `f3c3897e3010f4d659a38686537897e3fcbca8f582dbd5616d65db94fb1a85e2` |
| `src/credentials.ts`                | `8bc78f4390366f37a064ca4cc882f036e009eb7115e0224d622b9dcd8f0f92b3` |
| `src/utils/graphql.ts`              | `0eab8c64fabb80b8a3e5f58032de7bec1d3123a47e6a917b6dcb8bccd0f4d322` |

The manifest `rust/parity/runner/f06-teamref-frozen-cases.sha256` lists the SHA-256 of each of the 48 JSON cases and seven fixture files. Its own SHA-256 is `721d0976c9b8326d66f6b904937132500ee2d2ce484c070fd7ac5541023f5d3b`, pinned by `f06-teamref-frozen-cases.test.ts`. This includes the deliberately tracked `.env` with an empty `LINEAR_API_KEY` value.

Cases distinguish key > ID > name precedence; mixed-case direct UUID and uppercased URL UUID variables; response-order ambiguity; URL-to-name fallback; original URL versus rewritten-key error text; all-page suggestions (including an explicit null cursor); GraphQL failures; workspace source precedence, empty shadowing and three suggestion branches; JavaScript whitespace (`FEFF` versus `U+0085`); non-ASCII name matching; URL percent escapes, malformed escapes, WHATWG dot normalization, scheme-less input, default/nondefault port, userinfo and lookalike host. The fixture server uses a single `teams` backing field for both `teams` and aliased `teamById`, so a disjoint pair of response node sets cannot be proven in E0; F06-B's injected resolver tests own that case. Repeated-cursor infinite loops and malformed GraphQL payloads remain outside the finite schema-valid corpus.

The public carrier prefixes errors with `✗ Failed to fetch team members:` followed by one space. For resolver provenance, remove the prefix and that space, remove one terminal LF, then split once on LF followed by two spaces; hash the UTF-8 message and optional suggestion **without** separator or terminal LF. The following hashes bind resolver-level assertions in F06-A/B to the exact public error evidence. The per-case file hash in the manifest binds the full stdout/stderr/status and request fixture.

| Case                              | Resolver message SHA-256                                           | Suggestion SHA-256                                                 |
| --------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------ |
| f06e0-all-teams-error             | `94764fc57593a9b4546e92d03d97f9c9e46e5014259421bf15b44bf23d3cc14a` | —                                                                  |
| f06e0-ambiguous-name              | `55cdd4694815f78d68b75d159be7fcb77c07e740ddc49477ed9ad351608de85d` | `6a0e7b05cb5fa8edbc5be79551c6e11bb09cdd61bc54212c14b2639356e7d665` |
| f06e0-blank-feff-before-key       | `22df4e55d844165201a0728dfcfdbc81168eeccd4014cfda45010b3ac912c637` | `0f25dfec779f5299b8f6a8849581f83c412c5717d7af881df03f1af5148eab89` |
| f06e0-blank-space-before-key      | `22df4e55d844165201a0728dfcfdbc81168eeccd4014cfda45010b3ac912c637` | `0f25dfec779f5299b8f6a8849581f83c412c5717d7af881df03f1af5148eab89` |
| f06e0-dot-parent                  | `915e2c6c9b30765ce3aa4a252488b5bbddb25a1cc15ff0d02b4fa9c71d88e937` | `ca4086d64a66e7c63d12ef543ec6152b9ae10afd8fd6d7317e37fc1751b8a033` |
| f06e0-dot-percent                 | `440d0f2881813ae95db96914e201bccf32d2414fee3f5a33349b1cefdef142be` | `ca4086d64a66e7c63d12ef543ec6152b9ae10afd8fd6d7317e37fc1751b8a033` |
| f06e0-foreign-before-key          | `7eb9737762ab8be41da9204a61a1d5daa29a2e993e90cc34377eca0ea57af166` | `8b296f59793f6026e4f521bc1aa84b666a4e0bd2154193b24f1c4afdbec18b91` |
| f06e0-lookalike-fallthrough       | `d426b76cd39519aa8729d544d16d69322f8e6c9ff98caa7f283ea984d686ddbc` | `4e18e35bf86f57da558a12a60c2bf61a7a80e67c826d79f9de10eba78dda24c9` |
| f06e0-malformed-escape            | `ffd9649a21a79d6e539a390dc7870ce90622cb7d8909be2835afe43254e2e13a` | `ca4086d64a66e7c63d12ef543ec6152b9ae10afd8fd6d7317e37fc1751b8a033` |
| f06e0-matching-key-conflict       | `d6eba7b6eb20d18b67af73bd2274d705356e174f2f80c902ad10a4c371991a23` | —                                                                  |
| f06e0-miss-empty                  | `2c3963319774df4897c7682c75b29fd9f53afea29b125d15cb8d4316e2925a5d` | `4e18e35bf86f57da558a12a60c2bf61a7a80e67c826d79f9de10eba78dda24c9` |
| f06e0-miss-null-cursor            | `2c3963319774df4897c7682c75b29fd9f53afea29b125d15cb8d4316e2925a5d` | `15d290c8456c706b4dfff0db19bbef1bd07de347cf5780f587bd0ca59d8df0dc` |
| f06e0-miss-two-pages              | `2c3963319774df4897c7682c75b29fd9f53afea29b125d15cb8d4316e2925a5d` | `14c9f5798772784b15cd1423be28e88a783af85e6d6fd2235a4fa3d6abec6eab` |
| f06e0-nel-nonblank                | `acec2831108a19d860ae4d0bbc2a0ff9d8eab6dd946f8791cc4bcde9f74fea4e` | `4e18e35bf86f57da558a12a60c2bf61a7a80e67c826d79f9de10eba78dda24c9` |
| f06e0-port-fallthrough            | `99a36b890f994a6adc58a0eac39d9b59eff76309920b8c5916aa1e4d0453833a` | `4e18e35bf86f57da558a12a60c2bf61a7a80e67c826d79f9de10eba78dda24c9` |
| f06e0-resolve-error               | `d3cf43c51e2aa06fbeff91c46cca8e1209c866b230be065a1127446ac8afba3b` | —                                                                  |
| f06e0-suggest-config-empty-key    | `7eb9737762ab8be41da9204a61a1d5daa29a2e993e90cc34377eca0ea57af166` | `0fa954d4b31c672f3232d6023a82987dd914d74d77508f09fe579d7b850a5482` |
| f06e0-suggest-config-key          | `5aa21f3fdbb4f21a9b8cf0f204b8f3be7ef2c005444e3bf78612e70291271f96` | `0522a24cf2a05bf28562f4fb6142eb76efd44e5289f28c14a2fff46e7c88f238` |
| f06e0-suggest-plain               | `7eb9737762ab8be41da9204a61a1d5daa29a2e993e90cc34377eca0ea57af166` | `f21352e7a6f27d06c8a370d15a14fe766cac7ac8910d51945a7782373dfd300a` |
| f06e0-suggest-raw-empty           | `7eb9737762ab8be41da9204a61a1d5daa29a2e993e90cc34377eca0ea57af166` | `8b296f59793f6026e4f521bc1aa84b666a4e0bd2154193b24f1c4afdbec18b91` |
| f06e0-unsupported-foreign         | `51968895a698230241059920978376f15bfa8786fb7572535cc1b6c3d1ec328e` | `ca4086d64a66e7c63d12ef543ec6152b9ae10afd8fd6d7317e37fc1751b8a033` |
| f06e0-untrimmed-text              | `a5586fd02b4aa49cc9291f8450b6d4592d09d25f931dc747a648aa038e000717` | `0afb444f23a799712c8c040f1ea6fee701b2786620f8ad26c4fb1eed07e7b7cd` |
| f06e0-unvalidated-default         | `92d7ef07545dafded49560607c886bea8d78c0f092e12b35a05f978b7cfee773` | `a83da2b699d344a310f65e5f5570d3f2e291e4d1fc53b15f893602c7187dbf76` |
| f06e0-url-ambiguity-key           | `7bc207c979e4e1c1869d53ca29016e85bb17f92b2461552007a47b794c099e31` | `6a0e7b05cb5fa8edbc5be79551c6e11bb09cdd61bc54212c14b2639356e7d665` |
| f06e0-url-miss-original           | `01e0649712ff6a6efc5d54991090fb0bdae6904a0e0224ddfe24bef911d2a05e` | `0afb444f23a799712c8c040f1ea6fee701b2786620f8ad26c4fb1eed07e7b7cd` |
| f06e0-userinfo-fallthrough        | `4067c9b6dc8b36f1585ccfe6cc0556df09a118f5914780f21ce18af31051ce8e` | `4e18e35bf86f57da558a12a60c2bf61a7a80e67c826d79f9de10eba78dda24c9` |
| f06e0-workspace-env-mismatch-case | `9b3a9d6fdfffa2a13b27ed624ed8e3e343b47bc2219677185a0c33dd1cd6b9d0` | `782ae5cc14030058f547b55850f36a4f3102fc8d7c4c31944650a38c0332418f` |
| f06e0-wrong-kind-foreign          | `7eb9737762ab8be41da9204a61a1d5daa29a2e993e90cc34377eca0ea57af166` | `8b296f59793f6026e4f521bc1aa84b666a4e0bd2154193b24f1c4afdbec18b91` |

The exact interpreted-versus-compiled replay passed **48/48** cases before rebase, after rebasing onto integrated C002 `50b8cf14`, and after correcting the empty-config discriminator, with zero failure, unimplemented, or baseline drift. The staged Deno cache digest remained `6cf7fd5d357fa6478ed6cded0bc029022dbd04e1b9c99ad6a6dfda9fd556e56e` in both runs. After rebase, the focused strict-schema/hash test and `deno check`/`deno lint`/`deno fmt --check` passed. The complete post-rebase `deno task parity:test` suite passed **187/187**, including a final run after the discriminator and manifest update; the isolated pre-rebase suite had passed 184/184 after generating ignored GraphQL code. Three separate mutations of `f06e0-name` each failed as expected with one baseline drift: a one-byte stdout edit produced a stdout mismatch; removal of `id:null` produced a GraphQL operation/variable mismatch; and one extra expected GraphQL request produced a fixture count mismatch (three expected, two observed). The ignored notebook holds the mutation inputs and reports.

Claude's independent plan review returned REVISE and identified fixture and branch mistakes before tracked case generation: empty-string positional bypasses `findTeam`, raw empty env key violates fixture schema, aliased team fields share server data, whitespace CLI workspace affects later key selection, and WHATWG removes dot segments. The revised microplan in the ignored E0 notebook records how these findings were resolved. Fresh Claude whole-diff review of the rebased change returned **SHIP**. Its one discriminating-case finding was fixed: `f06e0-workspace-empty-config` now uses a foreign URL, so an erroneous fallback to the stored `acme` default fails. Its state/count wording findings were fixed here and in `RIIR_STATE.md`. No Rust resolver implementation is part of this item.

No live Linear account, host credential, main push, or security work was used. This E0 corpus is separate from C010's final v3 command goldens and leaves F06-A/B/C010 pending.

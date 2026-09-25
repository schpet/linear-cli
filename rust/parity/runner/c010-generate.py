import json
import sys
from pathlib import Path

if len(sys.argv) != 2:
    raise SystemExit('usage: python3 c010-generate.py <private-output-dir>')
root = Path(sys.argv[1])
root.mkdir(parents=True, exist_ok=True)
(root / 'fixtures/c010-project').mkdir(parents=True, exist_ok=True)
(root / 'fixtures/c010-project/linear.toml').write_text('team_id = "proj"\n')
schema = 'eef86b69c116d6adcb4f3659c29f9eb1407f84846f03cfda0b6096a80df3729a'
resolve_doc = 'query ResolveTeam($reference: String!, $id: ID, $isUuid: Boolean!) { teams(filter: { or: [{ key: { eqIgnoreCase: $reference } }, { name: { eqIgnoreCase: $reference } }] }) { nodes { id key name } } teamById: teams(filter: { id: { eq: $id } }) @include(if: $isUuid) { nodes { id key name } } }'
member_doc = 'query GetTeamMembers($teamKey: String!, $includeDisabled: Boolean!, $first: Int, $after: String) { team(id: $teamKey) { members(includeDisabled: $includeDisabled, first: $first, after: $after) { nodes { id name displayName email active initials description timezone lastSeen statusEmoji statusLabel guest isAssignable admin owner isMe url } pageInfo { hasNextPage endCursor } } } }'
identity = {'authorization': 'lin_api_fake', 'userAgent': 'schpet-linear-cli/2.6.0', 'headers': {}}

def step(id, document, variables, response):
    return {'kind': 'graphql', 'id': id, 'operation': {'document': document, 'variables': variables}, 'identity': identity, 'response': response, 'effects': []}

def data(value):
    return {'kind': 'data', 'data': value}

def raw(value):
    return {'kind': 'transport', 'status': 200, 'headers': {'content-type': 'application/json'}, 'body': {'utf8': json.dumps({'data': value})}}

def resolve(ref='ENG', key='ENG', name='Engineering', uuid=False):
    nodes = [{'id': ref if uuid else 'team-eng-id', 'key': key, 'name': name}]
    body = {'teams': {'nodes': nodes}}
    return step('resolve-team', resolve_doc, {'reference': ref, 'id': ref if uuid else None, 'isUuid': uuid}, data(body))

def member(id, name='Ada Lovelace', display='Ada', active=True, **kwargs):
    item = {'id': id, 'name': name, 'displayName': display, 'email': 'ada@example.invalid', 'active': active, 'initials': 'AL', 'description': None, 'timezone': None, 'lastSeen': None, 'statusEmoji': None, 'statusLabel': None, 'guest': False, 'isAssignable': True, 'admin': False, 'owner': False, 'isMe': False, 'url': 'https://linear.app/acme/profiles/' + id}
    item.update(kwargs)
    return item

def page(id='members', nodes=None, more=False, cursor=None, after=None, all=False, key='ENG', response=None):
    variables = {'teamKey': key, 'includeDisabled': all, 'first': 100}
    if after is not None:
        variables['after'] = after
    if response is None:
        response = data({'team': {'members': {'nodes': nodes or [], 'pageInfo': {'hasNextPage': more, 'endCursor': cursor}}}})
    return step(id, member_doc, variables, response)

def write(id, reason, argv, steps=None, *, env=None, exit=0, stdout='', stderr='', cwd='empty', closure=None):
    e = {'HOME': '{{home}}', 'XDG_CONFIG_HOME': '{{configHome}}', 'APPDATA': '{{configHome}}', 'PATH': '{{bin}}', 'DENO_DIR': '{{denoDir}}', 'TZ': 'UTC', 'LANG': 'C.UTF-8', 'LINEAR_IGNORE_ENV_FILE': '1', 'NO_COLOR': '1', 'LINEAR_GRAPHQL_ENDPOINT': 'http://127.0.0.1:{{fixturePort}}/graphql' if steps else 'http://127.0.0.1:1/graphql'}
    if steps:
        e['LINEAR_API_KEY'] = 'lin_api_fake'
    if env:
        e.update(env)
    x = {'id': id, 'route': 'linear team members', 'reason': reason, 'argv': argv, 'stdin': {'utf8': ''}, 'cwdFixture': cwd, 'env': e, 'substitutions': ['home', 'configHome', 'bin', 'denoDir'] + (['fixturePort'] if steps else []), 'timeoutMs': 30000, 'outputCapBytes': 4194304, 'fixtureServer': None, 'expected': {'exit': {'code': exit}, 'stdout': {'mode': 'closed-at-start'} if closure else {'utf8': stdout}, 'stderr': {'utf8': stderr}, 'fileEffects': []}, 'deviation': None}
    if steps:
        x['graphql'] = {'path': '/graphql', 'schemaSha256': schema, 'expectedRequests': len(steps), 'initialRecords': {}, 'expectedRecords': {}, 'groups': [{'mode': 'ordered', 'steps': steps}]}
    (root / (id + '.json')).write_text(json.dumps(x, indent=2, ensure_ascii=False) + '\n')

simple = [member('ada')]
inactive = [member('ada', active=False)]
blank = [member('blank', name='Name Only', display='', initials='', email='', description='', timezone='', statusEmoji='', statusLabel='')]
markers = [member('marks', name='Marker Name', display='Marker Display', active=False, guest=True, isAssignable=False, admin=True, owner=True, isMe=True, initials='MN', description='Owner', timezone='Europe/Paris', statusEmoji='🔥', statusLabel='Focus', lastSeen='2026-01-02T03:04:05Z')]
sort = [member('z', display='zulu'), member('accent', display='Émile'), member('case-b', display='Ada'), member('case-a', display='ada'), member('empty', name='Zoe', display='')]

write('c010-help', 'Leaf help and options.', ['team', 'members', '--help'])
write('c010-alias-help', 'Parent alias routes to leaf help.', ['t', 'members', '--help'])
write('c010-bad-option', 'Unknown option parser failure.', ['team', 'members', '--bogus'], exit=2)
write('c010-surplus', 'Surplus positional parser failure.', ['team', 'members', 'ENG', 'EXTRA'], exit=2)
write('c010-no-key', 'No configured team errors before transport.', ['team', 'members'], exit=1)
write('c010-empty-team', 'Empty argument falls through to configured team selection.', ['team', 'members', ''], exit=1)
write('c010-no-credential', 'Valid explicit team requires a credential.', ['team', 'members', 'ENG'], exit=1)
write('c010-empty-text', 'Empty member connection in human format.', ['team', 'members', 'ENG'], [resolve(), page()])
write('c010-empty-json', 'Empty member connection preserves GraphQL envelope.', ['team', 'members', 'ENG', '--json'], [resolve(), page()])
write('c010-alias-json', 'Parent alias and short JSON flag.', ['t', 'members', 'ENG', '-j'], [resolve(), page()])
write('c010-explicit-over-env', 'Explicit team overrides sourced team key.', ['team', 'members', 'ENG', '--json'], [resolve(), page()], env={'LINEAR_TEAM_ID': 'OPS'})
write('c010-sourced-key', 'Sourced team key skips resolution and uppercases.', ['team', 'members', '--json'], [page(key='OPS')], env={'LINEAR_TEAM_ID': 'ops'})
write('c010-empty-sourced-key', 'Explicit empty team falls through to sourced key.', ['team', 'members', '', '--json'], [page(key='OPS')], env={'LINEAR_TEAM_ID': 'ops'})
write('c010-active-text', 'Human output includes full-name suffix and email.', ['team', 'members', 'ENG'], [resolve(), page(nodes=simple)])
write('c010-active-json', 'JSON retains all member fields and connection nesting.', ['team', 'members', 'ENG', '--json'], [resolve(), page(nodes=simple)])
write('c010-inactive-text', 'Only inactive nodes produce specialized empty message.', ['team', 'members', 'ENG'], [resolve(), page(nodes=inactive)])
write('c010-inactive-json', 'JSON filters inactive nodes but retains pageInfo.', ['team', 'members', 'ENG', '--json'], [resolve(), page(nodes=inactive)])
write('c010-all-text', 'Long all flag includes disabled in request and renderer.', ['team', 'members', 'ENG', '--all'], [resolve(), page(nodes=inactive, all=True)])
write('c010-all-json', 'Short all flag includes disabled in request and JSON.', ['team', 'members', 'ENG', '-a', '-j'], [resolve(), page(nodes=inactive, all=True)])
mixed = [member('same', name='Grace', display='Grace'), member('gone', display='Bob', active=False)]
write('c010-mixed-text', 'Human output counts active members after filtering and omits identical name suffix.', ['team', 'members', 'ENG'], [resolve(), page(nodes=mixed)])
write('c010-mixed-json', 'JSON drops only inactive nodes from a mixed connection.', ['team', 'members', 'ENG', '--json'], [resolve(), page(nodes=mixed)])
write('c010-blank-display', 'Empty displayName and optional strings.', ['team', 'members', 'ENG'], [resolve(), page(nodes=blank)])
write('c010-markers', 'All independent markers in exact order plus detail fields.', ['team', 'members', 'ENG', '--all'], [resolve(), page(nodes=markers, all=True)])
write('c010-markers-json', 'JSON retains populated optional fields and raw timestamp.', ['team', 'members', 'ENG', '--all', '--json'], [resolve(), page(nodes=[member('marks', name='Marker Name', display='Marker Display', active=False, guest=True, isAssignable=False, admin=True, owner=True, isMe=True, initials='MN', description='Owner', timezone='Europe/Paris', statusEmoji='🔥', statusLabel='Focus', lastSeen='2026-01-02T03:04:05.123Z')], all=True)])
write('c010-partial-status', 'Emoji alone suppresses status.', ['team', 'members', 'ENG'], [resolve(), page(nodes=[member('partial', statusEmoji='🔥')])])
percent = [member('percent', name='%d %c', display='100%% %s', description='%%')]
write('c010-percent-text', 'Human renderer preserves literal percent signs.', ['team', 'members', 'ENG'], [resolve(), page(nodes=percent)])
write('c010-percent-json', 'JSON preserves literal percent signs.', ['team', 'members', 'ENG', '--json'], [resolve(), page(nodes=percent)])
write('c010-sort-text', 'Locale collation, case ties, accents and empty displayName.', ['team', 'members', 'ENG'], [resolve(), page(nodes=sort)])
write('c010-sort-json', 'Same global sort applies to JSON.', ['team', 'members', 'ENG', '--json'], [resolve(), page(nodes=sort)])
write('c010-two-pages-json', 'Page cursor, final pageInfo and global sort.', ['team', 'members', 'ENG', '--json'], [resolve(), page('first', [member('z', display='Zulu')], True, 'cursor-1'), page('second', [member('a', display='Ada')], False, 'done', 'cursor-1')])
write('c010-two-pages-text', 'Human format uses all pages.', ['team', 'members', 'ENG'], [resolve(), page('first', [member('z', display='Zulu')], True, 'cursor-1'), page('second', [member('a', display='Ada')], False, None, 'cursor-1')])
write('c010-null-cursor', 'A missing cursor on a continuing page errors.', ['team', 'members', 'ENG', '--json'], [resolve(), page(more=True)], exit=1)
write('c010-repeat-cursor', 'A repeated second-page cursor errors.', ['team', 'members', 'ENG', '--json'], [resolve(), page('first', [], True, 'cursor-1'), page('second', [], True, 'cursor-1', 'cursor-1')], exit=1)
write('c010-empty-cursor', 'An empty initial cursor is accepted then repeated-cursor fails.', ['team', 'members', 'ENG', '--json'], [resolve(), page('first', [], True, ''), page('second', [], True, '', '')], exit=1)
write('c010-graphql-error', 'GraphQL member error receives command context.', ['team', 'members', 'ENG'], [resolve(), page(response={'kind': 'graphqlErrors', 'status': 200, 'data': {'team': {'members': {'nodes': [], 'pageInfo': {'hasNextPage': False, 'endCursor': None}}}}, 'errors': [{'message': 'Fake member lookup failed'}]})], exit=1)
write('c010-http-error', 'HTTP member error receives command context.', ['team', 'members', 'ENG'], [resolve(), page(response={'kind': 'transport', 'status': 503, 'headers': {}, 'body': {'utf8': 'service unavailable'}})], exit=1)
write('c010-closed-text', 'Closed stdout does not change request sequence.', ['team', 'members', 'ENG'], [resolve(), page(nodes=simple)], closure=True)
write('c010-date-los-angeles', 'Absolute lastSeen renders in a second pinned timezone.', ['team', 'members', 'ENG', '--all'], [resolve(), page(nodes=markers, all=True)], env={'TZ': 'America/Los_Angeles'})
invalid_date = member('invalid', lastSeen='not-a-date')
write('c010-invalid-date', 'Raw GraphQL body preserves invalid DateTime for renderer behavior.', ['team', 'members', 'ENG'], [resolve(), page(response=raw({'team': {'members': {'nodes': [invalid_date], 'pageInfo': {'hasNextPage': False, 'endCursor': None}}}}))])
extra = member('extra'); extra['extraField'] = 'passes through'
write('c010-extra-field', 'Deno passes an unknown member field through to JSON.', ['team', 'members', 'ENG', '--json'], [resolve(), page(response=raw({'team': {'members': {'nodes': [extra], 'pageInfo': {'hasNextPage': False, 'endCursor': None}}}}))])
wrong = member('wrong', active='yes')
write('c010-wrong-type', 'Deno keeps truthy string active and passes it through to JSON.', ['team', 'members', 'ENG', '--json'], [resolve(), page(response=raw({'team': {'members': {'nodes': [wrong], 'pageInfo': {'hasNextPage': False, 'endCursor': None}}}}))])
null_display = [member('first'), member('second', display=None)]
write('c010-null-display', 'Raw required-null displayName causes sort failure with two nodes.', ['team', 'members', 'ENG'], [resolve(), page(response=raw({'team': {'members': {'nodes': null_display, 'pageInfo': {'hasNextPage': False, 'endCursor': None}}}}))], exit=1)
write('c010-null-team', 'Raw null team causes member access failure.', ['team', 'members', 'ENG'], [resolve(), page(response=raw({'team': None}))], exit=1)
write('c010-project-key', 'Project config team_id supplies the key without resolution.', ['team', 'members', '--json'], [page(key='PROJ')], cwd='c010-project')
write('c010-empty-env-hides-project', 'Empty sourced environment key hides project config.', ['team', 'members'], env={'LINEAR_TEAM_ID': ''}, cwd='c010-project', exit=1)

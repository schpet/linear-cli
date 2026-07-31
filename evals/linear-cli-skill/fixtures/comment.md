Reproduced this on staging with the following steps:

1. Log in via SSO
2. Open the billing page in a second tab
3. Refresh the first tab

The session cookie is refreshed with a mismatched domain, which is why the redirect loops. Fix candidate: pin the cookie domain in the auth callback.

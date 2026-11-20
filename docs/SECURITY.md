# Mnemosyne security and privacy boundaries

1. **Local first:** Prototype imported originals are copied into browser IndexedDB, not transmitted to a server. Original source files remain untouched. Browser storage can be cleared; users should retain independent backups.
2. **No hidden uploads:** Supabase is currently migration-only. Never automatically sync when credentials become available; require explicit opt-in UI with confirmation and quota estimates.
3. **RLS per table:** Every cloud table is owner-scoped. Composite foreign keys also require matching ownership for linked assets/albums/people/tags, protecting against cross-user references.
4. **Private buckets:** Two storage buckets are private; objects must begin with `${auth.uid()}/` (the account UUID as the first path segment). No public bucket. Never expose `service_role` or secret keys to browser clients.
5. **Threat model before launch:** XSS, malicious image decoder inputs, large image decompression bombs, file permission grants, data exfiltration, sharing/ACL bypass, sync conflicts, token storage, cloud spend and ransomware-style destructive operations.
6. **Deletion and recovery:** Deleting from the app currently means soft-deleting a catalog item, not erasing source files. Permanent delete should always be explicit and reversible via backup when feasible.
7. **Supabase deployment:** Create a new project; test RLS using two independent authenticated accounts; run Supabase advisors; review grants, bucket policies, quotas and object key path behavior before enabling cloud operations.
8. **Native scanning:** The current `inspect_directory` command accepts an arbitrary local path, and must not be exposed to untrusted remote content. In production, issue scoped directory access through OS-native picker and capability-bound tokens.
9. **Not audited:** This is a foundation for further security review, not a security certification or E2EE claim.

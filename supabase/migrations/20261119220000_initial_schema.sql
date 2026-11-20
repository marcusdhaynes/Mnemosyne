-- MNEMOSYNE / Dedicated Supabase project. Never run against an existing unrelated app.
-- All cloud storage and synchronization are OPT-IN; local library does not require an account.
create extension if not exists pgcrypto with schema extensions;

create table public.photo_libraries (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  name text not null default 'My Library' check (char_length(name) between 1 and 120),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique(id, owner_id)
);
create index photo_libraries_owner_idx on public.photo_libraries(owner_id);

create table public.photo_assets (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  library_id uuid not null,
  filename text not null check (char_length(filename) between 1 and 1024),
  media_kind text not null check (media_kind in ('image','video')),
  mime_type text,
  byte_size bigint not null default 0 check (byte_size >= 0),
  sha256 text check (sha256 is null or sha256 ~ '^[0-9a-f]{64}$'),
  perceptual_hash text,
  width integer check (width is null or width > 0),
  height integer check (height is null or height > 0),
  captured_at timestamptz,
  imported_at timestamptz not null default now(),
  favorite boolean not null default false,
  original_storage_path text,
  thumbnail_storage_path text,
  metadata jsonb not null default '{}'::jsonb,
  deleted_at timestamptz,
  foreign key (library_id,owner_id) references public.photo_libraries(id,owner_id) on delete cascade,
  unique(id,owner_id)
);
create index photo_assets_owner_time_idx on public.photo_assets(owner_id, captured_at desc, id);
create index photo_assets_owner_hash_idx on public.photo_assets(owner_id, sha256) where sha256 is not null;
create index photo_assets_owner_deleted_idx on public.photo_assets(owner_id,deleted_at);
create index photo_assets_library_idx on public.photo_assets(library_id);

create table public.photo_albums (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  library_id uuid not null,
  name text not null check (char_length(name) between 1 and 120),
  description text,
  cover_asset_id uuid,
  created_at timestamptz not null default now(),
  foreign key (library_id, owner_id) references public.photo_libraries(id,owner_id) on delete cascade,
  unique(id,owner_id)
);
create index photo_albums_owner_idx on public.photo_albums(owner_id);

create table public.photo_album_items (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  album_id uuid not null,
  asset_id uuid not null,
  position bigint not null default 0,
  added_at timestamptz not null default now(),
  foreign key (album_id,owner_id) references public.photo_albums(id,owner_id) on delete cascade,
  foreign key (asset_id,owner_id) references public.photo_assets(id,owner_id) on delete cascade,
  unique (album_id,asset_id)
);
create index photo_album_items_asset_idx on public.photo_album_items(asset_id);
create index photo_album_items_album_sort_idx on public.photo_album_items(album_id,position);

create table public.photo_tags (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  name text not null check (char_length(name) between 1 and 100),
  created_at timestamptz not null default now(),
  unique(id,owner_id), unique(owner_id,name)
);
create table public.photo_asset_tags (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  asset_id uuid not null,
  tag_id uuid not null,
  source text not null default 'manual' check (source in ('manual','filename','model')),
  confidence real check (confidence is null or confidence between 0 and 1),
  foreign key (asset_id,owner_id) references public.photo_assets(id,owner_id) on delete cascade,
  foreign key (tag_id,owner_id) references public.photo_tags(id,owner_id) on delete cascade,
  unique(asset_id,tag_id)
);
create index photo_asset_tags_tag_idx on public.photo_asset_tags(tag_id);

create table public.photo_people (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  display_name text not null check (char_length(display_name) between 1 and 120),
  cover_asset_id uuid,
  created_at timestamptz not null default now(),
  unique(id,owner_id)
);
create table public.photo_face_matches (
  id uuid primary key default gen_random_uuid(),
  owner_id uuid not null references auth.users(id) on delete cascade,
  asset_id uuid not null,
  person_id uuid not null,
  bounding_box jsonb,
  confidence real check (confidence is null or confidence between 0 and 1),
  reviewed boolean not null default false,
  foreign key (asset_id,owner_id) references public.photo_assets(id,owner_id) on delete cascade,
  foreign key (person_id,owner_id) references public.photo_people(id,owner_id) on delete cascade,
  unique(asset_id,person_id)
);

create table public.photo_sync_events (
  id bigint generated always as identity primary key,
  owner_id uuid not null references auth.users(id) on delete cascade,
  device_id uuid not null,
  entity_type text not null check (entity_type in ('asset','album','album_item','tag','asset_tag','person','face')),
  entity_id uuid not null,
  operation text not null check (operation in ('upsert','delete')),
  changed_at timestamptz not null default now(),
  revision bigint not null default 1 check (revision > 0)
);
create index photo_sync_events_cursor_idx on public.photo_sync_events(owner_id,id);

-- Explicit ownership policies for each table; users cannot reference someone else's rows.
do $$
declare t text;
begin
  foreach t in array array['photo_libraries','photo_assets','photo_albums','photo_album_items','photo_tags','photo_asset_tags','photo_people','photo_face_matches','photo_sync_events']
  loop
    execute format('alter table public.%I enable row level security',t);
    execute format('create policy %I on public.%I for select to authenticated using (owner_id = (select auth.uid()))',t||'_own_select',t);
    execute format('create policy %I on public.%I for insert to authenticated with check (owner_id = (select auth.uid()))',t||'_own_insert',t);
    execute format('create policy %I on public.%I for update to authenticated using (owner_id = (select auth.uid())) with check (owner_id = (select auth.uid()))',t||'_own_update',t);
    execute format('create policy %I on public.%I for delete to authenticated using (owner_id = (select auth.uid()))',t||'_own_delete',t);
    execute format('grant select,insert,update,delete on public.%I to authenticated',t);
  end loop;
end $$;
grant usage,select on all sequences in schema public to authenticated;

-- Private buckets: storage access is namespaced by the authenticated account's UID.
insert into storage.buckets (id,name,public,file_size_limit) values
 ('mnemosyne-originals','mnemosyne-originals',false,536870912),
 ('mnemosyne-previews','mnemosyne-previews',false,20971520)
on conflict (id) do nothing;

create policy mnemosyne_storage_read on storage.objects for select to authenticated
  using (bucket_id in ('mnemosyne-originals','mnemosyne-previews')
         and (storage.foldername(name))[1] = (select auth.uid())::text);
create policy mnemosyne_storage_insert on storage.objects for insert to authenticated
  with check (bucket_id in ('mnemosyne-originals','mnemosyne-previews')
         and (storage.foldername(name))[1] = (select auth.uid())::text);
create policy mnemosyne_storage_update on storage.objects for update to authenticated
  using (bucket_id in ('mnemosyne-originals','mnemosyne-previews')
         and (storage.foldername(name))[1] = (select auth.uid())::text)
  with check (bucket_id in ('mnemosyne-originals','mnemosyne-previews')
         and (storage.foldername(name))[1] = (select auth.uid())::text);
create policy mnemosyne_storage_delete on storage.objects for delete to authenticated
  using (bucket_id in ('mnemosyne-originals','mnemosyne-previews')
         and (storage.foldername(name))[1] = (select auth.uid())::text);

-- The remaining protection: never send secrets, raw embeddings, or face vectors to Supabase
-- unless the user explicitly opts into a reviewed cloud feature.

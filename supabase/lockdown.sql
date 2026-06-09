-- Lock the table so the publishable/anon key has ZERO direct access.
-- After this, the ONLY way to touch `userbase` is through the Edge Function
-- (which runs with service_role and bypasses RLS). A reverser who extracts the
-- publishable key cannot read, insert, update or — crucially — flip is_allowed
-- back to true. The blacklist becomes uncircumventable.
--
-- Run this in: Supabase Dashboard → SQL Editor → New query → Run.

-- 1) Make sure Row Level Security is ON.
alter table public.userbase enable row level security;
-- Also force it for the table owner, so nothing slips through.
alter table public.userbase force row level security;

-- 2) Drop EVERY existing policy on the table (names may differ from earlier).
do $$
declare p record;
begin
  for p in
    select policyname from pg_policies
    where schemaname = 'public' and tablename = 'userbase'
  loop
    execute format('drop policy if exists %I on public.userbase', p.policyname);
  end loop;
end $$;
-- (No policies + RLS on = anon/authenticated are fully denied. service_role bypasses RLS.)

-- 3) Belt and suspenders: revoke direct table privileges from the public API roles.
revoke all on public.userbase from anon;
revoke all on public.userbase from authenticated;

-- 4) Clean up any test/probe rows.
delete from public.userbase
where pc_id in ('__test__', '__test2__', '__probe_does_not_exist__', '0');

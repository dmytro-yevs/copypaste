-- ---------------------------------------------------------------------------
-- Local development seed. Applied by `supabase db reset`, never by
-- `supabase db push`.
--
-- Two accounts, so that "device A cannot see device B's rows" can be exercised
-- against a real PostgREST rather than only in the SQL assertions:
--
--     dev-a@example.test / copypaste-dev
--     dev-b@example.test / copypaste-dev
--
-- No clipboard rows are seeded, and none can be: a row's payload is sealed
-- under an Argon2id key derived from a passphrase this database has never seen
-- and must never see. Rows come from a client, or from `supabase/dev/smoke.sh`
-- which writes deliberately-undecryptable placeholder ciphertext.
-- ---------------------------------------------------------------------------

do $seed$
declare
    account   record;
    has_provider_id boolean;
begin
    select exists (
        select 1 from information_schema.columns
         where table_schema = 'auth' and table_name = 'identities'
           and column_name = 'provider_id'
    ) into has_provider_id;

    for account in
        select * from (values
            ('11111111-1111-4111-8111-111111111111'::uuid, 'dev-a@example.test'),
            ('22222222-2222-4222-8222-222222222222'::uuid, 'dev-b@example.test')
        ) as t(id, email)
    loop
        if exists (select 1 from auth.users u where u.id = account.id) then
            continue;
        end if;

        insert into auth.users (
            instance_id, id, aud, role, email, encrypted_password,
            email_confirmed_at, created_at, updated_at,
            raw_app_meta_data, raw_user_meta_data,
            confirmation_token, recovery_token, email_change, email_change_token_new
        ) values (
            '00000000-0000-0000-0000-000000000000',
            account.id,
            'authenticated',
            'authenticated',
            account.email,
            extensions.crypt('copypaste-dev', extensions.gen_salt('bf')),
            now(), now(), now(),
            '{"provider":"email","providers":["email"]}'::jsonb,
            '{}'::jsonb,
            '', '', '', ''
        );

        -- GoTrue grew `identities.provider_id` partway through its life; the
        -- local stack's version depends on the CLI. Branch rather than pin.
        if has_provider_id then
            insert into auth.identities (user_id, provider_id, identity_data, provider,
                                         last_sign_in_at, created_at, updated_at)
            values (account.id, account.id::text,
                    jsonb_build_object('sub', account.id::text, 'email', account.email),
                    'email', now(), now(), now());
        else
            insert into auth.identities (user_id, identity_data, provider,
                                         last_sign_in_at, created_at, updated_at)
            values (account.id,
                    jsonb_build_object('sub', account.id::text, 'email', account.email),
                    'email', now(), now(), now());
        end if;
    end loop;
end
$seed$;

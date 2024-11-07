INSERT INTO
    roles (name)
VALUES
    ('Admin'),
    ('User')
ON CONFLICT DO NOTHING;

-- password は Pa55w0rd
INSERT INTO
    users (name, email, password_hash, role_id)
SELECT
    'Eleazar Fig',
    'eleazar.fig@example.com',
    '$2b$12$KqcafKr1APdgw6ceB/bTQe47mk94zsPCBj4UIUomwP2ZnK7dAPWQa',
    role_id
FROM
    roles
WHERE
    name LIKE 'Admin';

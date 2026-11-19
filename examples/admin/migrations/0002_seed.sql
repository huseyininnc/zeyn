INSERT INTO customers (name, email, city, segment, active, since)
SELECT f.first || ' ' || f.last,
       lower(f.first) || '.' || lower(f.last) || i || '@example.com',
       (ARRAY['Istanbul', 'Ankara', 'Izmir', 'Berlin', 'London', 'Paris', 'Madrid', 'Vienna'])[i % 8 + 1],
       CASE i % 5 WHEN 0 THEN 'Partner' WHEN 1 THEN 'Business' WHEN 2 THEN 'Business' ELSE 'Retail' END,
       i % 11 <> 0,
       CURRENT_DATE - (i * 3 % 720)::int
FROM generate_series(1, 500) AS i,
     LATERAL (SELECT (ARRAY['Ada', 'Grace', 'Alan', 'Linus', 'Barbara', 'Edsger', 'Margaret', 'Dennis', 'Frances', 'Ken', 'Radia', 'Niklaus'])[i % 12 + 1] AS first,
                     (ARRAY['Lovelace', 'Hopper', 'Turing', 'Torvalds', 'Liskov', 'Dijkstra', 'Hamilton', 'Ritchie', 'Allen', 'Thompson'])[i % 10 + 1] AS last) AS f
ORDER BY i;

INSERT INTO products (sku, name, category, price, price_currency, stock, active)
SELECT 'SKU-' || lpad(i::text, 4, '0'),
       (ARRAY['Laptop', 'Monitor', 'Keyboard', 'Licence', 'Support plan', 'Printer paper', 'Toner', 'Router', 'Training day', 'Cloud backup'])[i % 10 + 1] || ' ' || i,
       CASE WHEN i % 10 <= 2 THEN 'Hardware' WHEN i % 10 = 3 THEN 'Software' WHEN i % 10 IN (4, 8) THEN 'Service' ELSE 'Supplies' END,
       (i % 37 + 1) * 2490,
       'TRY',
       i * 13 % 90,
       i % 13 <> 0
FROM generate_series(1, 200) AS i
ORDER BY i;

INSERT INTO orders (number, customer_id, customer_name, placed, status, items, total, total_currency)
SELECT 'SO-' || lpad(o.i::text, 5, '0'),
       c.id,
       c.name,
       CURRENT_DATE - o.age::int,
       CASE WHEN o.i % 17 = 0 THEN 'Cancelled'
            WHEN o.age <= 2 THEN 'New'
            WHEN o.age <= 6 THEN 'Processing'
            WHEN o.age <= 13 THEN 'Shipped'
            ELSE 'Delivered' END,
       o.i % 6 + 1,
       o.i::bigint * 7919 % 450000 + 9900,
       'TRY'
FROM (SELECT i, i * 37 % 365 AS age FROM generate_series(1, 5000) AS i) AS o
JOIN customers c ON c.id = o.i * 7 % 500 + 1
ORDER BY o.i;

INSERT INTO users (name, email, role, active)
SELECT f.first || ' ' || f.last,
       lower(f.first) || '.' || lower(f.last) || (i + 1000) || '@example.com',
       CASE WHEN i <= 2 THEN 'Admin' WHEN i <= 6 THEN 'Manager' ELSE 'Viewer' END,
       i <> 12
FROM generate_series(1, 12) AS i,
     LATERAL (SELECT (ARRAY['Ada', 'Grace', 'Alan', 'Linus', 'Barbara', 'Edsger', 'Margaret', 'Dennis', 'Frances', 'Ken', 'Radia', 'Niklaus'])[(i + 1000) % 12 + 1] AS first,
                     (ARRAY['Lovelace', 'Hopper', 'Turing', 'Torvalds', 'Liskov', 'Dijkstra', 'Hamilton', 'Ritchie', 'Allen', 'Thompson'])[(i + 1000) % 10 + 1] AS last) AS f
ORDER BY i;

INSERT INTO settings (company, support_email, ship_within_days, low_stock, notify_new_orders, weekly_report)
VALUES ('Acme Trading', 'support@acme.example', 3, 10, TRUE, FALSE);

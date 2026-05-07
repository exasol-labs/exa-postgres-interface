import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.RowIdLifetime;
import java.sql.SQLException;
import java.sql.SQLFeatureNotSupportedException;
import java.sql.Statement;
import java.sql.Types;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;

public class PgJdbcCompatibilitySuite {
    private static final int SAMPLE_ROW_LIMIT = 5;

    public static void main(String[] args) throws Exception {
        Config config = Config.parse(args);
        Reporter reporter = new Reporter(config.openOutput());
        SampleNames sample = new SampleNames(config.catalog, config.schema, config.table, config.columnPattern);

        reporter.section("Connection");
        try (Connection conn = openConnection(config)) {
            DatabaseMetaData meta = conn.getMetaData();
            reporter.line("url=" + config.jdbcUrl);
            reporter.line("product=" + safeValue(meta.getDatabaseProductName()));
            reporter.line("product_version=" + safeValue(meta.getDatabaseProductVersion()));
            reporter.line("driver=" + safeValue(meta.getDriverName()) + " " + safeValue(meta.getDriverVersion()));
            reporter.line("catalog=" + safeValue(conn.getCatalog()));
        }

        runMetadataSweep(config, sample, reporter);
        runSqlProbes(config, sample, reporter);

        reporter.finish();
        if (config.strict && reporter.mustPassFailures > 0) {
            System.exit(1);
        }
    }

    private static Connection openConnection(Config config) throws SQLException {
        return DriverManager.getConnection(config.jdbcUrl, config.user, config.password);
    }

    private static void runMetadataSweep(Config config, SampleNames sample, Reporter reporter) throws Exception {
        reporter.section("DatabaseMetaData Sweep");
        try (Connection conn = openConnection(config)) {
            DatabaseMetaData meta = conn.getMetaData();
            List<Method> methods = new ArrayList<Method>(Arrays.asList(DatabaseMetaData.class.getMethods()));
            Collections.sort(methods, new Comparator<Method>() {
                @Override
                public int compare(Method left, Method right) {
                    int byName = left.getName().compareTo(right.getName());
                    if (byName != 0) {
                        return byName;
                    }
                    int byParamCount = Integer.compare(left.getParameterTypes().length, right.getParameterTypes().length);
                    if (byParamCount != 0) {
                        return byParamCount;
                    }
                    return left.toString().compareTo(right.toString());
                }
            });

            for (Method method : methods) {
                InvocationPlan plan;
                try {
                    plan = InvocationPlan.forMethod(method, sample);
                } catch (IllegalArgumentException ex) {
                    reporter.recordSkip("metadata", method.getName(), Expectation.EXPLORATORY,
                        "unsupported argument mapping: " + ex.getMessage());
                    continue;
                }

                try {
                    Object value = method.invoke(meta, plan.arguments);
                    reporter.recordPass("metadata", methodSignature(method), Expectation.EXPLORATORY,
                        describeReturnValue(value));
                } catch (InvocationTargetException ex) {
                    reporter.recordFailure("metadata", methodSignature(method), Expectation.EXPLORATORY,
                        ex.getCause() == null ? ex : ex.getCause());
                } catch (Throwable ex) {
                    reporter.recordFailure("metadata", methodSignature(method), Expectation.EXPLORATORY, ex);
                }
            }
        }
    }

    private static void runSqlProbes(Config config, SampleNames sample, Reporter reporter) throws Exception {
        reporter.section("SQL Probes");
        for (QueryProbe probe : QueryProbe.corpus(sample)) {
            if (!config.shouldRunPersona(probe.persona)) {
                continue;
            }

            try (Connection conn = openConnection(config)) {
                reporter.recordPass(
                    probe.persona,
                    probe.id,
                    probe.expectation,
                    executeProbe(conn, probe, sample)
                );
            } catch (Throwable ex) {
                reporter.recordFailure(probe.persona, probe.id, probe.expectation, ex);
            }
        }
    }

    private static String executeProbe(Connection conn, QueryProbe probe, SampleNames sample) throws SQLException {
        if (probe.expectFailure) {
            try {
                runSupportSql(conn, probe.setupSql);
                try (Statement stmt = conn.createStatement()) {
                    stmt.execute(probe.sql);
                }
                throw new RuntimeException("Expected SQL error but query succeeded for probe: " + probe.id);
            } catch (SQLException expected) {
                return "rejected_as_expected: " + expected.getMessage();
            }
        }
        try {
            runSupportSql(conn, probe.setupSql);
            if (probe.prepared) {
                try (PreparedStatement stmt = conn.prepareStatement(probe.sql)) {
                    probe.binder.bind(stmt, sample);
                    return describeExecution(stmt.execute(), stmt);
                }
            }
            try (Statement stmt = conn.createStatement()) {
                return describeExecution(stmt.execute(probe.sql), stmt);
            }
        } finally {
            runCleanupSql(conn, probe.cleanupSql);
        }
    }

    private static void runSupportSql(Connection conn, List<String> statements) throws SQLException {
        if (statements.isEmpty()) {
            return;
        }
        try (Statement stmt = conn.createStatement()) {
            for (String sql : statements) {
                stmt.execute(sql);
            }
        }
    }

    private static void runCleanupSql(Connection conn, List<String> statements) {
        if (statements.isEmpty()) {
            return;
        }
        try (Statement stmt = conn.createStatement()) {
            for (String sql : statements) {
                try {
                    stmt.execute(sql);
                } catch (SQLException ignored) {
                }
            }
        } catch (SQLException ignored) {
        }
    }

    private static String methodSignature(Method method) {
        StringBuilder sb = new StringBuilder();
        sb.append(method.getName()).append('(');
        Class<?>[] parameterTypes = method.getParameterTypes();
        for (int i = 0; i < parameterTypes.length; i++) {
            if (i > 0) {
                sb.append(", ");
            }
            sb.append(parameterTypes[i].getSimpleName());
        }
        sb.append(')');
        return sb.toString();
    }

    private static String describeReturnValue(Object value) throws SQLException {
        if (value == null) {
            return "value=null";
        }
        if (value instanceof ResultSet) {
            return describeResultSet((ResultSet) value);
        }
        if (value instanceof Connection) {
            Connection conn = (Connection) value;
            return "connection_class=" + conn.getClass().getName() + " catalog=" + safeValue(conn.getCatalog());
        }
        if (value instanceof RowIdLifetime) {
            return "value=" + ((RowIdLifetime) value).name();
        }
        return "value=" + sanitize(String.valueOf(value));
    }

    private static String describeResultSet(ResultSet rs) throws SQLException {
        try {
            ResultSetMetaData meta = rs.getMetaData();
            int columnCount = meta.getColumnCount();
            int rows = 0;
            List<String> samples = new ArrayList<String>();
            while (rows < SAMPLE_ROW_LIMIT && rs.next()) {
                rows++;
                samples.add(formatRow(rs, meta));
            }
            return "cols=" + columnCount + " rows_shown=" + rows + " sample=" + sanitize(String.join(" || ", samples));
        } finally {
            rs.close();
        }
    }

    private static String describeExecution(boolean hasResultSet, Statement stmt) throws SQLException {
        if (hasResultSet) {
            try (ResultSet rs = stmt.getResultSet()) {
                return "statement=resultset " + describeResultSet(rs);
            }
        }
        return "statement=update update_count=" + stmt.getUpdateCount()
            + " more_results=" + stmt.getMoreResults(Statement.CLOSE_ALL_RESULTS);
    }

    private static String formatRow(ResultSet rs, ResultSetMetaData meta) throws SQLException {
        StringBuilder sb = new StringBuilder();
        for (int i = 1; i <= meta.getColumnCount(); i++) {
            if (i > 1) {
                sb.append(" | ");
            }
            sb.append(meta.getColumnLabel(i)).append('=').append(sanitize(rs.getString(i)));
        }
        return sb.toString();
    }

    private static String sanitize(String value) {
        if (value == null) {
            return "null";
        }
        return value.replace('\n', ' ').replace('\r', ' ').replace('\t', ' ').trim();
    }

    private static String safeValue(String value) {
        return sanitize(value);
    }

    private interface StatementBinder {
        void bind(PreparedStatement stmt, SampleNames sample) throws SQLException;
    }

    private static final class NoOpBinder implements StatementBinder {
        static final NoOpBinder INSTANCE = new NoOpBinder();

        @Override
        public void bind(PreparedStatement stmt, SampleNames sample) throws SQLException {
        }
    }

    private enum Expectation {
        MUST_PASS,
        EXPLORATORY
    }

    private static final class QueryProbe {
        final String persona;
        final String id;
        final Expectation expectation;
        final boolean prepared;
        final String sql;
        final StatementBinder binder;
        final List<String> setupSql;
        final List<String> cleanupSql;
        final boolean expectFailure;

        QueryProbe(
            String persona,
            String id,
            Expectation expectation,
            boolean prepared,
            String sql,
            StatementBinder binder,
            List<String> setupSql,
            List<String> cleanupSql,
            boolean expectFailure
        ) {
            this.persona = persona;
            this.id = id;
            this.expectation = expectation;
            this.prepared = prepared;
            this.sql = sql;
            this.binder = binder;
            this.setupSql = setupSql;
            this.cleanupSql = cleanupSql;
            this.expectFailure = expectFailure;
        }

        static List<QueryProbe> corpus(SampleNames sample) {
            List<QueryProbe> probes = new ArrayList<QueryProbe>();

            probes.add(simple("baseline", "select-1", Expectation.MUST_PASS, "SELECT 1"));
            probes.add(simple(
                "baseline",
                "sample-conversion-query",
                Expectation.MUST_PASS,
                "SELECT order_id, order_ts::DATE AS order_date, amount::DECIMAL(18, 2) AS amount_eur "
                    + "FROM pg_demo.orders WHERE customer_name ILIKE 'acme%' ORDER BY order_id LIMIT 3"
            ));
            probes.add(simple(
                "baseline",
                "catalog-database-query",
                Expectation.MUST_PASS,
                "SELECT d.datname AS table_cat FROM pg_catalog.pg_database d ORDER BY d.datname"
            ));

            probes.add(simple("dbvisualizer", "pg-tables", Expectation.MUST_PASS,
                "select * from pg_tables where schemaname != 'pg_catalog'"));
            probes.add(simple("dbvisualizer", "information-schema-tables", Expectation.MUST_PASS,
                "select TABLE_NAME from INFORMATION_SCHEMA.TABLES "
                    + "where TABLE_CATALOG = 'exasol' and TABLE_SCHEMA = 'PG_DEMO' order by TABLE_NAME"));
            probes.add(simple("dbvisualizer", "information-schema-columns", Expectation.MUST_PASS,
                "select COLUMN_NAME from INFORMATION_SCHEMA.COLUMNS "
                    + "where TABLE_CATALOG = 'exasol' and TABLE_SCHEMA = 'PG_DEMO' and TABLE_NAME = 'ORDERS' "
                    + "order by COLUMN_NAME"));
            probes.add(simple("dbvisualizer", "pg-user", Expectation.MUST_PASS, "select * from pg_user"));
            probes.add(simple("dbvisualizer", "pg-group", Expectation.MUST_PASS, "select * from pg_group"));
            probes.add(simple("dbvisualizer", "pg-stat-activity", Expectation.MUST_PASS, "select * from pg_stat_activity"));
            probes.add(simple("dbvisualizer", "pg-locks", Expectation.MUST_PASS, "select * from pg_locks"));

            probes.add(simple("pgjdbc", "pg-settings-max-index-keys", Expectation.EXPLORATORY,
                "SELECT setting FROM pg_catalog.pg_settings WHERE name='max_index_keys'"));
            probes.add(simple("pgjdbc", "pg-type-name-length", Expectation.EXPLORATORY,
                "SELECT t.typlen FROM pg_catalog.pg_type t, pg_catalog.pg_namespace n "
                    + "WHERE t.typnamespace=n.oid AND t.typname='name' AND n.nspname='pg_catalog'"));

            probes.add(simple("metabase", "limit-zero-table-metadata", Expectation.EXPLORATORY,
                "SELECT * FROM pg_demo.orders LIMIT 0"));
            probes.add(simple("metabase", "limit-zero-subquery-metadata", Expectation.EXPLORATORY,
                "SELECT * FROM (SELECT order_id, amount::DECIMAL(18, 2) AS amount_eur "
                    + "FROM pg_demo.orders WHERE customer_name ILIKE 'acme%') q LIMIT 0"));
            probes.add(simple("metabase", "limit-one-cte-metadata", Expectation.EXPLORATORY,
                "WITH base AS (SELECT order_id, customer_name, amount FROM pg_demo.orders) SELECT * FROM base LIMIT 1"));
            probes.add(simple("metabase", "table-constraints", Expectation.EXPLORATORY,
                "SELECT constraint_name, table_name, constraint_type "
                    + "FROM information_schema.table_constraints "
                    + "WHERE table_catalog = 'exasol' AND table_schema = 'PG_DEMO' "
                    + "ORDER BY table_name, constraint_name"));
            probes.add(simple("metabase", "key-column-usage", Expectation.EXPLORATORY,
                "SELECT table_name, column_name, ordinal_position "
                    + "FROM information_schema.key_column_usage "
                    + "WHERE table_catalog = 'exasol' AND table_schema = 'PG_DEMO' "
                    + "ORDER BY table_name, ordinal_position"));

            probes.add(prepared("dbeaver", "database-lookup", Expectation.EXPLORATORY,
                "SELECT db.oid,db.* FROM pg_catalog.pg_database db WHERE datname=?",
                new StatementBinder() {
                    @Override
                    public void bind(PreparedStatement stmt, SampleNames names) throws SQLException {
                        stmt.setString(1, names.catalog);
                    }
                }));
            probes.add(simple("dbeaver", "schema-cache", Expectation.EXPLORATORY,
                "SELECT n.oid,n.*,d.description FROM pg_catalog.pg_namespace n "
                    + "LEFT OUTER JOIN pg_catalog.pg_description d "
                    + "ON d.objoid=n.oid AND d.objsubid=0 AND d.classoid='pg_namespace'::regclass "
                    + "ORDER BY nspname"));
            probes.add(prepared("dbeaver", "table-cache", Expectation.EXPLORATORY,
                "SELECT c.oid,c.*,d.description "
                    + "FROM pg_catalog.pg_class c "
                    + "LEFT OUTER JOIN pg_catalog.pg_description d "
                    + "ON d.objoid=c.oid AND d.objsubid=0 AND d.classoid='pg_class'::regclass "
                    + "WHERE c.relnamespace=(SELECT oid FROM pg_catalog.pg_namespace WHERE nspname=?) "
                    + "AND c.relkind not in ('i','I','c')",
                new StatementBinder() {
                    @Override
                    public void bind(PreparedStatement stmt, SampleNames names) throws SQLException {
                        stmt.setString(1, names.schema);
                    }
                }));
            probes.add(prepared("dbeaver", "column-cache", Expectation.EXPLORATORY,
                "SELECT c.relname,a.*,pg_catalog.pg_get_expr(ad.adbin, ad.adrelid, true) as def_value,dsc.description "
                    + "FROM pg_catalog.pg_attribute a "
                    + "INNER JOIN pg_catalog.pg_class c ON (a.attrelid=c.oid) "
                    + "LEFT OUTER JOIN pg_catalog.pg_attrdef ad ON (a.attrelid=ad.adrelid AND a.attnum = ad.adnum) "
                    + "LEFT OUTER JOIN pg_catalog.pg_description dsc ON (c.oid=dsc.objoid AND a.attnum = dsc.objsubid) "
                    + "WHERE NOT a.attisdropped AND c.relkind not in ('i','I','c') "
                    + "AND c.relnamespace=(SELECT oid FROM pg_catalog.pg_namespace WHERE nspname=?) "
                    + "ORDER BY a.attnum",
                new StatementBinder() {
                    @Override
                    public void bind(PreparedStatement stmt, SampleNames names) throws SQLException {
                        stmt.setString(1, names.schema);
                    }
                }));
            probes.add(prepared("dbeaver", "constraint-cache", Expectation.EXPLORATORY,
                "SELECT c.oid,c.*,t.relname as tabrelname,rt.relnamespace as refnamespace,d.description, "
                    + "case when c.contype='c' then \"substring\"(pg_get_constraintdef(c.oid), 7) else null end consrc_copy "
                    + "FROM pg_catalog.pg_constraint c "
                    + "INNER JOIN pg_catalog.pg_class t ON t.oid=c.conrelid "
                    + "LEFT OUTER JOIN pg_catalog.pg_class rt ON rt.oid=c.confrelid "
                    + "LEFT OUTER JOIN pg_catalog.pg_description d "
                    + "ON d.objoid=c.oid AND d.objsubid=0 AND d.classoid='pg_constraint'::regclass "
                    + "WHERE t.relnamespace=(SELECT oid FROM pg_catalog.pg_namespace WHERE nspname=?) "
                    + "ORDER BY c.oid",
                new StatementBinder() {
                    @Override
                    public void bind(PreparedStatement stmt, SampleNames names) throws SQLException {
                        stmt.setString(1, names.schema);
                    }
                }));
            probes.add(prepared("dbeaver", "index-cache", Expectation.EXPLORATORY,
                "SELECT i.*,i.indkey as keys,c.relname,c.relnamespace,c.relam,c.reltablespace,tc.relname as tabrelname,dsc.description, "
                    + "pg_catalog.pg_get_expr(i.indpred, i.indrelid) as pred_expr, "
                    + "pg_catalog.pg_get_expr(i.indexprs, i.indrelid, true) as expr "
                    + "FROM pg_catalog.pg_index i "
                    + "INNER JOIN pg_catalog.pg_class c ON c.oid=i.indexrelid "
                    + "INNER JOIN pg_catalog.pg_class tc ON tc.oid=i.indrelid "
                    + "LEFT OUTER JOIN pg_catalog.pg_description dsc ON i.indexrelid=dsc.objoid "
                    + "WHERE c.relnamespace=(SELECT oid FROM pg_catalog.pg_namespace WHERE nspname=?) "
                    + "ORDER BY tabrelname, c.relname",
                new StatementBinder() {
                    @Override
                    public void bind(PreparedStatement stmt, SampleNames names) throws SQLException {
                        stmt.setString(1, names.schema);
                    }
                }));

            probes.add(simple("analyst", "grouping-and-having", Expectation.EXPLORATORY,
                "SELECT customer_name, SUM(amount) AS total_amount "
                    + "FROM pg_demo.orders GROUP BY customer_name HAVING SUM(amount) > 50 ORDER BY total_amount DESC"));
            probes.add(simple("analyst", "window-running-total", Expectation.EXPLORATORY,
                "SELECT order_id, amount, "
                    + "SUM(amount) OVER (ORDER BY order_id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS running_amount "
                    + "FROM pg_demo.orders ORDER BY order_id"));
            probes.add(simple("analyst", "limit-offset", Expectation.EXPLORATORY,
                "SELECT order_id FROM pg_demo.orders ORDER BY order_id LIMIT 2 OFFSET 1"));
            probes.add(simple("analyst", "distinct-on", Expectation.EXPLORATORY,
                "SELECT DISTINCT ON (customer_name) customer_name, order_id "
                    + "FROM pg_demo.orders ORDER BY customer_name, order_id DESC"));
            probes.add(simple("analyst", "filter-clause", Expectation.EXPLORATORY,
                "SELECT COUNT(*) FILTER (WHERE amount > 100) AS gt_100 FROM pg_demo.orders"));
            probes.add(simple("analyst", "array-any", Expectation.EXPLORATORY,
                "SELECT * FROM pg_demo.orders WHERE customer_name = ANY(ARRAY['Acme GmbH','Beta AG'])"));
            probes.add(simple("analyst", "unnest-array", Expectation.EXPLORATORY,
                "SELECT unnest(ARRAY[1,2,3]) AS n"));
            probes.add(simple("analyst", "jsonb-build-object", Expectation.EXPLORATORY,
                "SELECT jsonb_build_object('customer', customer_name) FROM pg_demo.orders LIMIT 1"));

            probes.add(simple("dml", "insert-select-noop", Expectation.EXPLORATORY,
                "INSERT INTO pg_demo.orders (order_id, customer_name, amount, order_ts) "
                    + "SELECT 999999, 'Gateway Probe', 1.23, CURRENT_TIMESTAMP WHERE 1 = 0"));
            probes.add(simple("dml", "update-noop", Expectation.EXPLORATORY,
                "UPDATE pg_demo.orders SET amount = amount WHERE 1 = 0"));
            probes.add(simple("dml", "delete-noop", Expectation.EXPLORATORY,
                "DELETE FROM pg_demo.orders WHERE 1 = 0"));
            probes.add(simple("dml", "merge-noop", Expectation.EXPLORATORY,
                "MERGE INTO pg_demo.orders AS target "
                    + "USING (SELECT 999999 AS order_id, 'Gateway Probe' AS customer_name, 1.23 AS amount, CURRENT_TIMESTAMP AS order_ts) src "
                    + "ON target.order_id = src.order_id "
                    + "WHEN MATCHED AND 1 = 0 THEN UPDATE SET amount = target.amount"));

            probes.add(withSetupAndCleanup("ddl", "create-drop-table", Expectation.EXPLORATORY,
                "CREATE TABLE " + sample.scratchTable + " (id INTEGER)",
                cleanup("DROP TABLE " + sample.scratchTable)));
            probes.add(withSetupAndCleanup("ddl", "create-drop-view", Expectation.EXPLORATORY,
                "CREATE VIEW " + sample.scratchView + " AS SELECT 1 AS id",
                cleanup("DROP VIEW " + sample.scratchView)));
            probes.add(withSetupAndCleanup(
                "ddl",
                "alter-table-add-column",
                Expectation.EXPLORATORY,
                "ALTER TABLE " + sample.scratchTable + " ADD COLUMN note VARCHAR(20)",
                setup("CREATE TABLE " + sample.scratchTable + " (id INTEGER)"),
                cleanup("DROP TABLE " + sample.scratchTable)
            ));
            probes.add(withSetupAndCleanup(
                "ddl",
                "truncate-table",
                Expectation.EXPLORATORY,
                "TRUNCATE TABLE " + sample.scratchTable,
                setup(
                    "CREATE TABLE " + sample.scratchTable + " (id INTEGER)",
                    "INSERT INTO " + sample.scratchTable + " VALUES (1)"
                ),
                cleanup("DROP TABLE " + sample.scratchTable)
            ));
            probes.add(withSetupAndCleanup(
                "ddl",
                "drop-table",
                Expectation.EXPLORATORY,
                "DROP TABLE " + sample.scratchTable,
                setup("CREATE TABLE " + sample.scratchTable + " (id INTEGER)")
            ));

            probes.add(simple("transaction", "begin", Expectation.EXPLORATORY, "BEGIN"));
            probes.add(simple("transaction", "commit", Expectation.EXPLORATORY, "COMMIT"));
            probes.add(simple("transaction", "rollback", Expectation.EXPLORATORY, "ROLLBACK"));
            probes.add(withSetupAndCleanup(
                "transaction",
                "savepoint",
                Expectation.EXPLORATORY,
                "SAVEPOINT gateway_probe_sp",
                setup("BEGIN"),
                cleanup("ROLLBACK")
            ));
            probes.add(withSetupAndCleanup(
                "transaction",
                "rollback-to-savepoint",
                Expectation.EXPLORATORY,
                "ROLLBACK TO SAVEPOINT gateway_probe_sp",
                setup("BEGIN", "SAVEPOINT gateway_probe_sp"),
                cleanup("ROLLBACK")
            ));
            probes.add(withSetupAndCleanup(
                "transaction",
                "release-savepoint",
                Expectation.EXPLORATORY,
                "RELEASE SAVEPOINT gateway_probe_sp",
                setup("BEGIN", "SAVEPOINT gateway_probe_sp"),
                cleanup("ROLLBACK")
            ));

            probes.add(simple("session", "set-application-name", Expectation.EXPLORATORY,
                "SET application_name = 'pg-jdbc-compat-suite'"));
            probes.add(simple("session", "show-server-version", Expectation.EXPLORATORY, "SHOW server_version"));
            probes.add(simple("session", "reset-application-name", Expectation.EXPLORATORY, "RESET application_name"));

            // set-search-path-single: SET single schema and verify current_schema() reflects it
            probes.add(withSetupAndCleanup(
                "session", "set-search-path-single", Expectation.MUST_PASS,
                "SELECT current_schema()",
                setup("SET search_path = \"PG_DEMO\""),
                Collections.<String>emptyList()
            ));

            // set-search-path-multi: multi-schema SET must be rejected with SQL error
            probes.add(expectingFailure(
                "session", "set-search-path-multi",
                "SET search_path TO pg_demo, pg_catalog"
            ));

            // reset-search-path: RESET search_path is a no-op (success)
            probes.add(simple("session", "reset-search-path", Expectation.MUST_PASS,
                "RESET search_path"));

            // show-search-path: SHOW search_path after SET returns the opened schema
            probes.add(withSetupAndCleanup(
                "session", "show-search-path", Expectation.MUST_PASS,
                "SHOW search_path",
                setup("SET search_path = \"PG_DEMO\""),
                Collections.<String>emptyList()
            ));

            // set-search-path-missing-schema: OPEN SCHEMA on nonexistent schema returns SQL error
            probes.add(expectingFailure(
                "session", "set-search-path-missing-schema",
                "SET search_path = \"DOES_NOT_EXIST\""
            ));

            probes.add(simple("utility", "explain-select", Expectation.EXPLORATORY,
                "EXPLAIN SELECT * FROM pg_demo.orders"));
            probes.add(simple("utility", "explain-analyze-select", Expectation.EXPLORATORY,
                "EXPLAIN ANALYZE SELECT * FROM pg_demo.orders"));
            probes.add(simple("utility", "lock-table", Expectation.EXPLORATORY,
                "LOCK TABLE pg_demo.orders IN ACCESS SHARE MODE"));
            probes.add(simple("utility", "copy-to-stdout", Expectation.EXPLORATORY,
                "COPY (SELECT order_id FROM pg_demo.orders ORDER BY order_id) TO STDOUT"));
            probes.add(simple("utility", "vacuum", Expectation.EXPLORATORY, "VACUUM pg_demo.orders"));
            probes.add(simple("utility", "analyze", Expectation.EXPLORATORY, "ANALYZE pg_demo.orders"));

            return probes;
        }

        static QueryProbe simple(String persona, String id, Expectation expectation, String sql) {
            return new QueryProbe(
                persona,
                id,
                expectation,
                false,
                sql,
                NoOpBinder.INSTANCE,
                Collections.<String>emptyList(),
                Collections.<String>emptyList(),
                false
            );
        }

        static QueryProbe prepared(
            String persona,
            String id,
            Expectation expectation,
            String sql,
            StatementBinder binder
        ) {
            return new QueryProbe(
                persona,
                id,
                expectation,
                true,
                sql,
                binder,
                Collections.<String>emptyList(),
                Collections.<String>emptyList(),
                false
            );
        }

        static QueryProbe withSetupAndCleanup(
            String persona,
            String id,
            Expectation expectation,
            String sql,
            List<String> cleanupSql
        ) {
            return new QueryProbe(
                persona,
                id,
                expectation,
                false,
                sql,
                NoOpBinder.INSTANCE,
                Collections.<String>emptyList(),
                cleanupSql,
                false
            );
        }

        static QueryProbe withSetupAndCleanup(
            String persona,
            String id,
            Expectation expectation,
            String sql,
            List<String> setupSql,
            List<String> cleanupSql
        ) {
            return new QueryProbe(persona, id, expectation, false, sql, NoOpBinder.INSTANCE, setupSql, cleanupSql, false);
        }

        static QueryProbe expectingFailure(String persona, String id, String sql) {
            return new QueryProbe(persona, id, Expectation.MUST_PASS, false, sql,
                NoOpBinder.INSTANCE, Collections.<String>emptyList(), Collections.<String>emptyList(), true);
        }

        static List<String> setup(String... statements) {
            return Arrays.asList(statements);
        }

        static List<String> cleanup(String... statements) {
            return Arrays.asList(statements);
        }
    }

    private static final class InvocationPlan {
        final Object[] arguments;

        InvocationPlan(Object[] arguments) {
            this.arguments = arguments;
        }

        static InvocationPlan forMethod(Method method, SampleNames sample) {
            Class<?>[] parameterTypes = method.getParameterTypes();
            Object[] arguments = new Object[parameterTypes.length];
            for (int i = 0; i < parameterTypes.length; i++) {
                arguments[i] = defaultArgument(method, i, parameterTypes[i], sample);
            }
            return new InvocationPlan(arguments);
        }

        private static Object defaultArgument(Method method, int index, Class<?> type, SampleNames sample) {
            String name = method.getName();
            if (type == String.class) {
                return defaultStringArgument(name, index, sample);
            }
            if (type == boolean.class) {
                return Boolean.valueOf(defaultBooleanArgument(name, index));
            }
            if (type == int.class) {
                return Integer.valueOf(defaultIntArgument(name, index));
            }
            if (type == String[].class) {
                if ("getTables".equals(name)) {
                    return new String[] {"TABLE", "VIEW"};
                }
                throw new IllegalArgumentException("unmapped String[] for " + name);
            }
            if (type == int[].class) {
                if ("getUDTs".equals(name)) {
                    return new int[] {Types.STRUCT, Types.DISTINCT, Types.JAVA_OBJECT};
                }
                throw new IllegalArgumentException("unmapped int[] for " + name);
            }
            if (type == Class.class) {
                return DatabaseMetaData.class;
            }
            throw new IllegalArgumentException("unmapped type " + type.getName() + " for " + name);
        }

        private static String defaultStringArgument(String methodName, int index, SampleNames sample) {
            if ("getSchemas".equals(methodName) && index == 0) {
                return sample.catalog;
            }
            if ("getSchemas".equals(methodName) && index == 1) {
                return "%";
            }
            if ("getTables".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : "%";
            }
            if ("getColumns".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : index == 2 ? sample.table : sample.columnPattern;
            }
            if ("getColumnPrivileges".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : index == 2 ? sample.table : sample.columnPattern;
            }
            if ("getTablePrivileges".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : "%";
            }
            if ("getBestRowIdentifier".equals(methodName)
                || "getVersionColumns".equals(methodName)
                || "getPrimaryKeys".equals(methodName)
                || "getImportedKeys".equals(methodName)
                || "getExportedKeys".equals(methodName)
                || "getIndexInfo".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : sample.table;
            }
            if ("getCrossReference".equals(methodName)) {
                if (index == 0 || index == 3) {
                    return sample.catalog;
                }
                if (index == 1 || index == 4) {
                    return sample.schema;
                }
                return sample.table;
            }
            if ("getUDTs".equals(methodName) || "getSuperTypes".equals(methodName) || "getAttributes".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : "%";
            }
            if ("getSuperTables".equals(methodName) || "getProcedures".equals(methodName) || "getFunctions".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : "%";
            }
            if ("getProcedureColumns".equals(methodName) || "getFunctionColumns".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : "%";
            }
            if ("getPseudoColumns".equals(methodName)) {
                return index == 0 ? sample.catalog : index == 1 ? sample.schema : index == 2 ? sample.table : sample.columnPattern;
            }
            throw new IllegalArgumentException("unmapped String for " + methodName + " arg " + index);
        }

        private static boolean defaultBooleanArgument(String methodName, int index) {
            if ("getBestRowIdentifier".equals(methodName)) {
                return true;
            }
            if ("getIndexInfo".equals(methodName)) {
                return false;
            }
            return false;
        }

        private static int defaultIntArgument(String methodName, int index) {
            if ("getBestRowIdentifier".equals(methodName) && index == 3) {
                return DatabaseMetaData.bestRowSession;
            }
            if ("supportsConvert".equals(methodName)) {
                return index == 0 ? Types.VARCHAR : Types.DECIMAL;
            }
            if ("supportsTransactionIsolationLevel".equals(methodName)) {
                return Connection.TRANSACTION_READ_COMMITTED;
            }
            if ("supportsResultSetType".equals(methodName)
                || "ownUpdatesAreVisible".equals(methodName)
                || "ownDeletesAreVisible".equals(methodName)
                || "ownInsertsAreVisible".equals(methodName)
                || "othersUpdatesAreVisible".equals(methodName)
                || "othersDeletesAreVisible".equals(methodName)
                || "othersInsertsAreVisible".equals(methodName)
                || "updatesAreDetected".equals(methodName)
                || "deletesAreDetected".equals(methodName)
                || "insertsAreDetected".equals(methodName)) {
                return ResultSet.TYPE_FORWARD_ONLY;
            }
            if ("supportsResultSetConcurrency".equals(methodName)) {
                return index == 0 ? ResultSet.TYPE_FORWARD_ONLY : ResultSet.CONCUR_READ_ONLY;
            }
            if ("supportsResultSetHoldability".equals(methodName)) {
                return ResultSet.HOLD_CURSORS_OVER_COMMIT;
            }
            throw new IllegalArgumentException("unmapped int for " + methodName + " arg " + index);
        }
    }

    private static final class SampleNames {
        final String catalog;
        final String schema;
        final String table;
        final String columnPattern;
        final String scratchTable;
        final String scratchView;

        SampleNames(String catalog, String schema, String table, String columnPattern) {
            this.catalog = catalog;
            this.schema = schema;
            this.table = table;
            this.columnPattern = columnPattern;
            String suffix = Long.toString(System.currentTimeMillis());
            this.scratchTable = schema + ".GATEWAY_COMPAT_" + suffix;
            this.scratchView = schema + ".GATEWAY_COMPAT_V_" + suffix;
        }
    }

    private static final class Config {
        final String jdbcUrl;
        final String user;
        final String password;
        final String catalog;
        final String schema;
        final String table;
        final String columnPattern;
        final boolean strict;
        final Set<String> personas;
        final String outputPath;

        Config(
            String jdbcUrl,
            String user,
            String password,
            String catalog,
            String schema,
            String table,
            String columnPattern,
            boolean strict,
            Set<String> personas,
            String outputPath
        ) {
            this.jdbcUrl = jdbcUrl;
            this.user = user;
            this.password = password;
            this.catalog = catalog;
            this.schema = schema;
            this.table = table;
            this.columnPattern = columnPattern;
            this.strict = strict;
            this.personas = personas;
            this.outputPath = outputPath;
        }

        static Config parse(String[] args) {
            if (args.length < 3) {
                throw new IllegalArgumentException(
                    "usage: PgJdbcCompatibilitySuite <jdbc-url> <user> <password> [--catalog=exasol] "
                        + "[--schema=PG_DEMO] [--table=ORDERS] [--column-pattern=%] "
                        + "[--personas=baseline,dbvisualizer,pgjdbc,metabase,dbeaver,analyst,dml,ddl,transaction,session,utility] "
                        + "[--strict] [--output=/path/report.txt]"
                );
            }

            String catalog = "exasol";
            String schema = "PG_DEMO";
            String table = "ORDERS";
            String columnPattern = "%";
            boolean strict = false;
            String output = null;
            Set<String> personas = new LinkedHashSet<String>();
            personas.add("all");

            for (int i = 3; i < args.length; i++) {
                String arg = args[i];
                if ("--strict".equals(arg)) {
                    strict = true;
                } else if (arg.startsWith("--catalog=")) {
                    catalog = arg.substring("--catalog=".length());
                } else if (arg.startsWith("--schema=")) {
                    schema = arg.substring("--schema=".length());
                } else if (arg.startsWith("--table=")) {
                    table = arg.substring("--table=".length());
                } else if (arg.startsWith("--column-pattern=")) {
                    columnPattern = arg.substring("--column-pattern=".length());
                } else if (arg.startsWith("--personas=")) {
                    personas.clear();
                    for (String persona : arg.substring("--personas=".length()).split(",")) {
                        if (!persona.trim().isEmpty()) {
                            personas.add(persona.trim().toLowerCase(Locale.ROOT));
                        }
                    }
                } else if (arg.startsWith("--output=")) {
                    output = arg.substring("--output=".length());
                } else {
                    throw new IllegalArgumentException("unknown argument: " + arg);
                }
            }

            return new Config(args[0], args[1], args[2], catalog, schema, table, columnPattern, strict, personas, output);
        }

        boolean shouldRunPersona(String persona) {
            return personas.contains("all") || personas.contains(persona.toLowerCase(Locale.ROOT));
        }

        PrintStream openOutput() throws Exception {
            if (outputPath == null || outputPath.isEmpty()) {
                return System.out;
            }
            OutputStream out = new FileOutputStream(outputPath);
            return new PrintStream(out, true, "UTF-8");
        }
    }

    private static final class Reporter {
        final PrintStream out;
        int mustPassFailures;
        int exploratoryFailures;
        int mustPassPasses;
        int exploratoryPasses;
        int skips;

        Reporter(PrintStream out) {
            this.out = out;
        }

        void section(String title) {
            out.println("== " + title + " ==");
        }

        void line(String line) {
            out.println(line);
        }

        void recordPass(String group, String id, Expectation expectation, String detail) {
            if (expectation == Expectation.MUST_PASS) {
                mustPassPasses++;
            } else {
                exploratoryPasses++;
            }
            out.println("PASS [" + expectation + "] " + group + "/" + id + " " + detail);
        }

        void recordFailure(String group, String id, Expectation expectation, Throwable failure) {
            Throwable root = rootCause(failure);
            StringBuilder detail = new StringBuilder();
            if (root instanceof SQLException) {
                SQLException sql = (SQLException) root;
                detail.append("sqlState=").append(sanitize(sql.getSQLState()))
                    .append(" message=").append(sanitize(sql.getMessage()));
            } else if (root instanceof SQLFeatureNotSupportedException) {
                detail.append("sqlFeatureNotSupported message=").append(sanitize(root.getMessage()));
            } else {
                detail.append("message=").append(sanitize(root.getMessage()));
            }

            if (expectation == Expectation.MUST_PASS) {
                mustPassFailures++;
            } else {
                exploratoryFailures++;
            }
            out.println("FAIL [" + expectation + "] " + group + "/" + id + " " + detail.toString());
        }

        void recordSkip(String group, String id, Expectation expectation, String reason) {
            skips++;
            out.println("SKIP [" + expectation + "] " + group + "/" + id + " reason=" + sanitize(reason));
        }

        void finish() {
            section("Summary");
            line("must_pass_passes=" + mustPassPasses);
            line("must_pass_failures=" + mustPassFailures);
            line("exploratory_passes=" + exploratoryPasses);
            line("exploratory_failures=" + exploratoryFailures);
            line("skips=" + skips);
            if (out != System.out) {
                out.close();
            }
        }

        private static Throwable rootCause(Throwable failure) {
            Throwable current = failure;
            while (current.getCause() != null && current.getCause() != current) {
                current = current.getCause();
            }
            return current;
        }
    }
}

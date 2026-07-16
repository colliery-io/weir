-- Demo database + table for the mssql connector integration test (WEIR-T-0161).
IF DB_ID('weir_demo') IS NULL
    CREATE DATABASE weir_demo;
GO
USE weir_demo;
GO
IF OBJECT_ID('dbo.contacts', 'U') IS NOT NULL
    DROP TABLE dbo.contacts;
GO
CREATE TABLE dbo.contacts (
    id          INT           NOT NULL PRIMARY KEY,
    email       NVARCHAR(200) NOT NULL,
    full_name   NVARCHAR(200) NULL,
    active      BIT           NOT NULL,
    updated_at  DATETIME2     NOT NULL
);
GO
INSERT INTO dbo.contacts (id, email, full_name, active, updated_at) VALUES
    (1, 'ada@weir.test', 'Ada Weir', 1, '2026-01-01T00:00:00'),
    (2, 'cy@weir.test',  'Cy Weir',  1, '2026-01-02T00:00:00'),
    (3, 'del@weir.test', 'Del Weir', 0, '2026-01-03T00:00:00');
GO

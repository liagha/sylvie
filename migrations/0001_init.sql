create table system (
    key text primary key,
    value blob not null
);

create table users (
    id text primary key,
    username text unique not null,
    record blob not null,
    created text not null
);

create table devices (
    id text primary key,
    owner text not null references users(id) on delete cascade,
    name text not null,
    created text not null,
    revoked text
);

create table sessions (
    hash text primary key,
    device text not null references devices(id) on delete cascade,
    created text not null
);

create table secrets (
    owner text not null references users(id) on delete cascade,
    name text not null,
    data blob not null,
    created text not null,
    updated text not null,
    primary key (owner, name)
);

create table files (
    id text primary key,
    owner text not null references users(id) on delete cascade,
    name text not null,
    size integer not null,
    hash text not null,
    path text not null,
    created text not null,
    updated text not null
);

/*
Copyright (c) 2019 llk89.

 This program is free software: you can redistribute it and/or modify
 it under the terms of the GNU Affero General Public License as
 published by the Free Software Foundation, either version 3 of the
 License, or (at your option) any later version.

 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU Affero General Public License for more details.

 You should have received a copy of the GNU Affero General Public License
 along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

drop procedure if exists setup_1;

delimiter //

create procedure setup_1()
  modifies sql data
begin
  create table if not exists repo_ids
  (
    repo_id        bigint unsigned not null
      primary key,
    course_uid     binary(16)      not null,
    assignment_uid binary(16)      not null,
    name           varchar(255)    not null,
    constraint course_uid
      unique (course_uid, assignment_uid, name)
  );

  create table if not exists uid
  (
    uid      bigint unsigned not null
      primary key,
    username varchar(255)    not null,
    constraint username
      unique (username)
  );

  create table if not exists uuids
  (
    gitlab_id bigint(64) unsigned not null
      primary key,
    uuid      binary(16)          not null
  );

  create table if not exists version
  (
    id int(7) unsigned not null
      primary key
  );

  insert ignore into version (id) values (0);
end//

delimiter ;

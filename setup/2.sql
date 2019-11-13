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

drop procedure if exists setup_2;
drop procedure if exists setup_2_;
delimiter //

create procedure setup_2()
  modifies sql data
begin
  create table if not exists version
  (
    id int(7) unsigned not null
      primary key
  );
  set @self = (select count(*) from version where id = 1);
  if (@self = 0) then
    call setup_2_();
  end if;
end//

create procedure setup_2_()
  modifies sql data
begin

  set @parent = (select count(*) from version where id = 0);
  if (@parent = 0) then
    call setup_1();
  end if;

  alter table repo_ids
    add constraint repo_ids_assignment_uid_uuids_uuid_fk
      foreign key (assignment_uid) references uuids (gitlab_id)
        on update cascade on delete cascade;

  alter table repo_ids
    add constraint repo_ids_course_uid_uuids_uuid_fk
      foreign key (course_uid) references uuids (gitlab_id)
        on update cascade on delete cascade;

  insert into version(id) VALUES (1);
end //

delimiter ;

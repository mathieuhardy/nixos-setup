# Auto-generated, do not edit !
{ config, ... }:

{
  networking.hostId = "082dbc0f";

  fileSystems."data_1" = {
    device = "/dev/disk/by-id/mmc-SU08G_0x21a906b7-part2";
  };

  fileSystems."data_2" = {
    device = "/dev/mapper/data_2";

    encrypted = {
      enable = true;
      blkdev = "/dev/disk/by-id/mmc-SU08G_0x21a906b7-part3";
      label = "data_2";
      keyFile = "/etc/secrets/disks/key_file";
    };
  };

  fileSystems."system" = {
    device = "/dev/mapper/system";

    encrypted = {
      enable = true;
      blkdev = "/dev/disk/by-id/mmc-SU08G_0x21a906b7-part4";
      label = "system";
      keyFile = "/etc/secrets/disks/key_file";
    };
  };
};
# Auto-generated, do not edit !
{ config, ... }:

{
  boot = {
    initrd = {
      luks.devices."data_2" = {
        device = "/dev/disk/by-id/mmc-SU08G_0x21a906b7-part3";
        keyFile = "/key_file";
        allowDiscards = true;
        preLVM = true;
      };

      luks.devices."system" = {
        device = "/dev/disk/by-id/mmc-SU08G_0x21a906b7-part4";
        keyFile = "/key_file";
        allowDiscards = true;
        preLVM = true;
      };

      secrets = {
        "/key_file" = "/etc/secrets/disks/key_file";
      };
    };
  };
};
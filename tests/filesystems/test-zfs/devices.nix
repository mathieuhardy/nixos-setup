# Auto-generated, do not edit !
{ config, ... }:

{
  boot = {
    supportedFilesystems = ["zfs"];

    initrd = {
      supportedFilesystems = ["zfs"];

      luks.devices."bank_system" = {
        device = "/dev/disk/by-id/mmc-SU08G_0x21a906b7-part5";
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
# Auto-generated, do not edit !
{ config, ... }:

{
  networking.hostId = "082dbc0f";

  fileSystems."/data_1" = {
    device = "bank_data/data_1";
    fsType = "zfs";
  };

  fileSystems."/useless" = {
    device = "bank_system/useless";
    fsType = "zfs";
  };

  fileSystems."/useless2" = {
    device = "bank_system/useless2";
    fsType = "zfs";
  };
};
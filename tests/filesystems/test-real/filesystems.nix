# Auto-generated, do not edit !
{ config, ... }:

{
  networking.hostId = "082dbc0f";

  fileSystems."/" = {
    device = "pool/root";
    fsType = "zfs";
  };
};
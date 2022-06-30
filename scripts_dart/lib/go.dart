import 'dart:typed_data';
import 'dart:convert';
import 'dart:io';
import 'package:ic_tools/ic_tools.dart';
import 'package:ic_tools/common.dart' as common;
import 'package:ic_tools/candid.dart';
import 'package:ic_tools/tools.dart';
import 'package:crypto/crypto.dart';


Map controllers_json = jsonDecode(utf8.decode(File('../../controllers_and_canisters.json').readAsBytesSync()));

Caller controller = CallerEd25519(
    public_key: Uint8List.fromList(controllers_json['controller']!['pub_key']!.cast<int>()),
    private_key: Uint8List.fromList(controllers_json['controller']!['s_key']!.cast<int>()),
);

Caller controller2 = CallerEd25519(
    public_key: Uint8List.fromList(controllers_json['controller2']!['pub_key']!.cast<int>()),
    private_key: Uint8List.fromList(controllers_json['controller2']!['s_key']!.cast<int>()),
);

Caller controller3 = CallerEd25519(
    public_key: Uint8List.fromList(controllers_json['controller3']!['pub_key']!.cast<int>()),
    private_key: Uint8List.fromList(controllers_json['controller3']!['s_key']!.cast<int>()),
);



//Canister canister = Canister(Principal('thp4z-laaaa-aaaam-qaaea-cai'));

Canister canister = Canister(Principal('bayhi-7yaaa-aaaai-qahca-cai'));

//Canister canister2 = Canister(Principal('hjgka-ziaaa-aaaam-qaeoa-cai')); // controller is the cts

Canister canister2 = Canister(Principal('mscqy-haaaa-aaaai-aahhq-cai'));

Canister canister3 = Canister(Principal('ha4iv-6iaaa-aaaah-aapjq-cai'));

Canister controller3_user_canister = Canister(Principal('woddh-aqaaa-aaaal-aazqq-cai'));






Future<void> main(List<String> arguments) async {
    
    String first_command = arguments[0];

    if (first_command == 'create_controller') {
        await create_controller();
    }
    
    else if (first_command == 'canister_status') {
        await canister_status();
    }

    else if (first_command == 'top_up_canister') {
        await top_up_canister(double.parse(arguments[1]));
    }

    else if (first_command == 'put_code_on_the_canister') {
        await put_code_on_the_canister(arguments[1]);
    }

    else if (first_command == 'create_canister') {
        await create_canister(double.parse(arguments[1]));
    }

    else if (first_command == 'put_frontcode_build_web') {
        await put_frontcode_build_web();
    }
    
    else if (first_command == 'clear_frontcode_files') {
        await clear_frontcode_files();
    }

    else if (first_command == 'change_canister_settings') {
        await change_canister_settings();
    }








    else if (first_command == 'call_canister_topup_balance') {
        await call_canister_topup_balance();
    }
    
    else if (first_command == 'call_canister_new_user') {
        await call_canister_new_user();
    }

    else if (first_command == 'call_canister_find_user_canister') {
        await call_canister_find_user_canister();
    }

    else if (first_command == 'call_user_canister_user_cycles_balance') {
        await call_user_canister_user_cycles_balance();
    }

    else if (first_command == 'call_user_canister_user_icp_balance') {
        await call_user_canister_user_icp_balance();
    }

    else if (first_command == 'call_user_canister_user_download_cycles_transfers_in') {
        await call_user_canister_user_download_cycles_transfers_in(int.parse(arguments[1]));
    }

    else if (first_command == 'call_user_canister_user_transfer_cycles') {
        await call_user_canister_user_transfer_cycles();
    }

    else if (first_command == 'call_user_canister_user_download_cycles_transfers_out') {
        await call_user_canister_user_download_cycles_transfers_out(int.parse(arguments[1]));
    }














 





    else if (first_command == 'call_canister_see_balance') {
        bool cts_balance = false;
        if (arguments.length >= 2 && arguments[1] == 'cts') {
            cts_balance = true;
        }
        await call_canister_see_balance(cts_balance);
    }

    else if (first_command == 'call_canister_topup_balance') {
        await call_canister_topup_balance();
    }
    
    else if (first_command == 'call_canister_convert_icp_balance_for_the_cycles_with_the_cmc_rate') {
        await call_canister_convert_icp_balance_for_the_cycles_with_the_cmc_rate(double.parse(arguments[1]));
    }
    
    else if (first_command == 'call_canister_collect_balance') {
        late Variant param;
        if (arguments[1] == 'cycles') {
            param = Variant.oftheMap({
                'cycles_payout': Record.oftheMap({
                    'cycles': Nat(int.parse(arguments[2])),
                    'payout_cycles_transfer_canister': PrincipalCandid(Principal(arguments[3])).candid
                })
            });
        } else if (arguments[1] == 'icp') {
            param = Variant.oftheMap({
                'icp_payout': Record.oftheMap({
                    'icp': Record.oftheMap({
                        'e8s': Nat64((double.parse(arguments[2]) * 100000000).toInt())
                    }),
                    'payout_icp_id': Blob(hexstringasthebytes(arguments[3]))
                })
            });
        }

        await call_canister_collect_balance(c_forwards([param]));
    }

    else if (first_command == 'purchase_cycles_bank') {
        await purchase_cycles_bank(arguments[1]);
    }

    else if (first_command == 'see_cycles_transfer_purchases') {
        await see_cycles_transfer_purchases(Nat(0));
    }
    
    else if (first_command == 'see_cycles_bank_purchases') {
        await see_cycles_bank_purchases(Nat(0));
    }









    else if (first_command == 'call_canister_controller_see_new_canisters') {
        await call_canister_controller_see_new_canisters();
    }

    else if (first_command == 'controller_put_new_canisters') {
        await controller_put_new_canisters(arguments[1].split(',').map<PrincipalReference>((String pstring)=>Principal(pstring).candid));
    }

    else if (first_command == 'controller_see_users_map_canisters') {
        await controller_see_users_map_canisters();
    }

    else if (first_command == 'controller_see_stable_size') {
        await controller_see_stable_size();
    }

    else if (first_command == 'controller_see_new_canister_status') {
        await controller_see_new_canister_status(Principal(arguments[1]));
    }

    else if (first_command == 'controller_put_umc_code') {
        await controller_put_umc_code();
    }
    
    else if (first_command == 'controller_put_user_canister_code') {
        await controller_put_user_canister_code();
    }

    else if (first_command == 'controller_put_ctc_code') {
        await controller_put_ctc_code();
    }



    else if (first_command == 'controller_deposit_cycles') {
        await controller_deposit_cycles(Nat(BigInt.parse(arguments[1])), Principal(arguments[2]));
    }

    else if (first_command == 'controller_see_metrics') {
        await controller_see_metrics();
    }
    
    else if (first_command == 'set_test_canisters_code') {
        await set_test_canisters_code(arguments[1]);
    }

    else if (first_command == 'run_cycles_transfer_test_canisters') {
        await run_cycles_transfer_test_canisters();
    }

    else if (first_command == 'controller_cts_call_canister') {
        await controller_cts_call_canister();
    }
    
    else if (first_command == 'controller_see_new_users') {
        await controller_see_new_users();
    }
    
    else if (first_command == 'controller_download_state_snapshot_change_state_snapshot_and_put_state_snapshot') {
        await controller_download_state_snapshot_change_state_snapshot_and_put_state_snapshot();
    }

    else if (first_command == 'controller_create_state_snapshot') {
        await controller_create_state_snapshot();
    }

    else if (first_command == 'controller_re_store_cts_data_out_of_the_state_snapshot') {
        await controller_re_store_cts_data_out_of_the_state_snapshot();
    }

    else if (first_command == 'controller_upgrade_umcs') {
        Vector<PrincipalReference>? opt_umcs;
        if (arguments.length >= 2) {
            opt_umcs = Vector.oftheList<PrincipalReference>(arguments[1].split(',').map<PrincipalReference>((String pstring)=>Principal(pstring).candid).toList());
        }
        await controller_upgrade_umcs(opt_umcs);
    }

    else if (first_command == 'controller_upgrade_ucs_on_a_umc') {
        await controller_upgrade_ucs_on_a_umc();
    }

    else if (first_command == 'controller_put_uc_code_onto_the_umcs') {
        await controller_put_uc_code_onto_the_umcs();
    }



    












    else if (first_command == 'test') {
        // print(candid_text_hash('Err'));
        // print(candid_text_hash('cycles'));
        //print(candid_text_hash('Ok'));

        // print(c_forwards([Nat(BigInt.from(5))]));

        // print('cts: ${canister.principal.bytes}');
        
        //print(await common.transfer_icp(controller3, '9d67f85f54646c79a6e693b477bb0d2d161cb73e441bb885cebee568c2f4efc8', 0.33187868));
        
        
        for (Principal p in [controller.principal,controller2.principal,controller3.principal,canister.principal,controller3_user_canister.principal]) {
            print(p);
            print(p.icp_id());
            print(await common.check_icp_balance(p.icp_id()));
        }
        
        
        for (String t in [
            'Ok',
            'Err',
            /*
            'UsersMapCanistersFindUserCallFails',
            'PutNewUserIntoAUsersMapCanisterError',
            'CreateUserCanisterIcpTransferError',
            'CreateUserCanisterIcpTransferCallError',
            'CreateUserCanisterCmcNotifyError',
            'CreateUserCanisterCmcNotifyCallError',
            'IcpTransferCallError',
            'IcpTransferError',
            'UserCanisterUninstallCodeCallError',
            'UserCanisterCodeNotFound',
            'UserCanisterInstallCodeCallError',
            'UserCanisterStatusCallError',
            'UserCanisterModuleVerificationError',
            'UserCanisterStartCanisterCallError',
            'UserCanisterUpdateSettingsCallError',
            'UsersMapCanisterPutNewUserCallFail',
            'UsersMapCanisterPutNewUserError',
            'CreateNewUsersMapCanisterError',
            'MaxUsersMapCanisters',
            'CreateNewUsersMapCanisterLockIsOn',
            'GetNewCanisterError',
            'UsersMapCanisterCodeNotFound',
            'InstallCodeCallError',
            'UpgradeCodeCallError',
            */
            'running',
            'stopping',
            'stopped'
        ]) {
            print(candid_text_hash(t));
        }

        print(sha256.convert(File('../rust/target/wasm32-unknown-unknown/release/users_map_canister-o.wasm').readAsBytesSync()).bytes);




        /*

        /// certified data test
        Uint8List sponse = await common.cycles_mint.call(
            calltype: 'query',
            method_name: 'get_icp_xdr_conversion_rate',
        );
        // print(sponse);
        List<CandidType> cs = c_backwards(sponse);
        // print(cs);

        Record rc = cs[0] as Record;
        Record r = rc['data'] as Record;
        Uint8List certificate_bytes = Blob.oftheVector((rc['certificate'] as Vector).cast_vector<Nat8>()).bytes;
        print('certificate_bytes len: ${certificate_bytes.length}');
        Map certificate = cbor.cborbytesasadart(certificate_bytes);
        await verify_certificate(certificate);
        dynamic time = lookuppathvalueinaniccertificatetree(certificate['tree'], ['time']);
        BigInt btime = time is int ? BigInt.from(time) : time; //as BigInt
        if (btime < get_current_time_nanoseconds() - BigInt.from(30*1000000000)) { throw Exception('time is too old on the certificate'); }
        print(btime);

        Uint8List certified_data = lookuppathvalueinaniccertificatetree(certificate['tree'], ['canister', common.cycles_mint.principal.bytes, 'certified_data']);
        print(certified_data);

        List canister_hash_tree = cbor.cborbytesasadart((rc['hash_tree'] as Blob).bytes);
        // print(canister_hash_tree);

        Uint8List treeroothash = constructicsystemstatetreeroothash(canister_hash_tree);
        print(treeroothash);

        if (!aresamebytes(certified_data, treeroothash)) { throw Exception('certified data doesn\'t match the tree'); }

        Record certified_icpxdrrate = c_backwards(lookuppathvalueinaniccertificatetree(canister_hash_tree, ["ICP_XDR_CONVERSION_RATE"], 'blob'))[0] as Record;
        
        print(r['xdr_permyriad_per_icp']);
        print(r['timestamp_seconds']);
        print(certified_icpxdrrate['xdr_permyriad_per_icp']);
        print(certified_icpxdrrate['timestamp_seconds']);

        */







    }







    else {
        throw Exception('"$first_command" is not a known command.');
    }

}




Future<void> create_controller() async {
    CallerEd25519 controller = CallerEd25519.new_keys();
    print(controller);
    print(controller.principal.icp_id());
    print('pub: ${controller.public_key}');
    print('priv: ${controller.private_key}');
    
}



Future<void> create_canister(double create_icp) async {
    print(await common.check_icp_balance(controller.principal.icp_id()));
    Principal can_id = await common.create_canister(controller, create_icp);
    print(can_id);
    Canister can = Canister(can_id);
    print(await can.controllers());
    print(await common.check_icp_balance(controller.principal.icp_id()));
}

Future<void> canister_status() async {
    print(await common.check_canister_status(controller, canister2.principal));

}

Future<void> top_up_canister(double top_up_icp) async {
    print(await common.check_icp_balance(controller.principal.icp_id()));
    await common.top_up_canister(controller, top_up_icp, canister.principal);
    print(await common.check_icp_balance(controller.principal.icp_id()));

}

Future<void> put_code_on_the_canister(String mode) async {
    Principal put_code_on_the_canister_id = canister.principal;
    
    print('you are about to $mode code onto the canister: ${put_code_on_the_canister_id}\ntype: "$mode" to continue.');
    if (stdin.readLineSync() != mode) {
        throw Exception('$mode confirmation fail.');
    } 

    if (mode == 'upgrade') {
        print('stop_canister');
        Uint8List sponse = await common.management.call(
            calltype: CallType.call,
            method_name: 'stop_canister',
            caller: controller,
            put_bytes: c_forwards([
                Record.oftheMap({
                    'canister_id': put_code_on_the_canister_id.candid
                })
            ])
        );
        print(sponse);
        List<CandidType> cs = c_backwards(sponse);
        print(cs);
    }

    print(mode + ' code');
        
    Uint8List install_code_arg = c_forwards([
        Record.oftheMap({
            'controllers': Vector.oftheList<PrincipalReference>([controller.principal.candid, controller2.principal.candid, controller3.principal.candid]) 
        })
    ]);
    Uint8List upgrade_code_arg = c_forwards([
        
    ]);
    
    await common.put_code_on_the_canister(
        controller,
        put_code_on_the_canister_id,
        File('../rust/target/wasm32-unknown-unknown/release/cts-o.wasm').readAsBytesSync(),
        mode,
        ['install', 'reinstall'].contains(mode) ? install_code_arg : upgrade_code_arg
    );

    
    if (mode == 'upgrade') {
        print('start_canister');
        Uint8List sponse = await common.management.call(
            calltype: CallType.call,
            method_name: 'start_canister',
            caller: controller,
            put_bytes: c_forwards([
                Record.oftheMap({
                    'canister_id': put_code_on_the_canister_id.candid
                })
            ])
        );
        print(sponse);
        List<CandidType> cs = c_backwards(sponse);
        print(cs);
    }

    
    
    
}

Future<void> change_canister_settings() async {
    Uint8List change_canister_settings_sponse = await common.management.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'update_settings',
        put_bytes: c_forwards([
            Record.oftheMap({
                'canister_id': canister2.principal.candid,
                'settings': Record.oftheMap({
                    // freezing_threshold : opt nat,
                    'controllers' : Option(value: Vector.oftheList<PrincipalReference>([
                        controller.principal.candid, 
                        controller2.principal.candid,
                        controller3.principal.candid,
                        canister.principal.candid

                    ])),
                    // memory_allocation : opt nat,
                    // compute_allocation : opt nat,
                }) 
            }) 
        ])
    );
    print(c_backwards(change_canister_settings_sponse));
}











Future<void> call_canister_new_user() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'new_user',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);

}



Future<void> call_canister_find_user_canister() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'find_user_canister',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);

}



Future<void> call_user_canister_user_cycles_balance() async {
    Uint8List sponse = await controller3_user_canister.call(
        calltype: CallType.call,
        method_name: 'user_cycles_balance',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}


Future<void> call_user_canister_user_icp_balance() async {
    Uint8List sponse = await controller3_user_canister.call(
        calltype: CallType.call,
        method_name: 'user_icp_balance',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}


Future<void> call_user_canister_user_download_cycles_transfers_in(int chunk_i) async {
    Uint8List sponse = await controller3_user_canister.call(
        calltype: CallType.call,
        method_name: 'user_download_cycles_transfers_in',
        caller: controller3,
        put_bytes: c_forwards([Nat32(chunk_i)])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}

Future<void> call_user_canister_user_transfer_cycles() async {
    Uint8List sponse = await controller3_user_canister.call(
        calltype: CallType.call,
        method_name: 'user_transfer_cycles',
        caller: controller3,
        put_bytes: c_forwards([
            // sending cycles for myself, out then in
            Record.oftheMap({
                'cycles': Nat(5),
                'canister_id': canister.principal.candid,
                'cycles_transfer_memo': Variant.oftheMap({
                    'Blob': Blob([...utf8.encode('UT'), controller3.principal.bytes.length, ...controller3.principal.bytes ])
                })
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}

Future<void> call_user_canister_user_download_cycles_transfers_out(int chunk_i) async {
    Uint8List sponse = await controller3_user_canister.call(
        calltype: CallType.call,
        method_name: 'user_download_cycles_transfers_out',
        caller: controller3,
        put_bytes: c_forwards([Nat32(chunk_i)])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}






















Future<void> call_canister_see_balance([bool cts = false]) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: cts ? 'controller_see_balance' : 'see_balance',
        caller: controller,
    );
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
    print({
        'Ok': {
            'cycles_balance': ((cs[0] as Variant)['Ok'] as Record)['cycles_balance'],
            'icp_balance' : ((((cs[0] as Variant)['Ok'] as Record)['icp_balance'] as Record)['e8s'] as Nat64).value / 100000000
        }
    });


}

Future<void> call_canister_topup_balance() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'topup_balance',
        caller: controller3,
    );
    List<CandidType> cs = c_backwards(sponse); 
    Record topup_balance_data = cs[0] as Record;
    print({
        'topup_cycles_balance': {
            'topup_cycles_transfer_memo': bytesasahexstring((((topup_balance_data['topup_cycles_balance'] as Record)['topup_cycles_transfer_memo'] as Variant)['Blob'] as Blob).bytes),
        },
        'topup_icp_balance' : {
            'topup_icp_id' : bytesasahexstring(((topup_balance_data['topup_icp_balance'] as Record)['topup_icp_id'] as Blob).bytes)
        }
    });

}

Future<void> call_canister_convert_icp_balance_for_the_cycles_with_the_cmc_rate(double icp) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'convert_icp_balance_for_the_cycles_with_the_cmc_rate',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'icp': Record.oftheMap({
                    'e8s': Nat64((icp * 100000000).toInt())
                })
            })
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}





Future<void> call_canister_collect_balance(Uint8List params) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'collect_balance',
        caller: controller,
        put_bytes: params
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}



Future<void> purchase_cycles_bank(String cycles_or_icp_string) async {
    if (!['icp','cycles'].contains(cycles_or_icp_string) ) { 
        throw Exception('cycles or icp');
    }
    
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'purchase_cycles_bank',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'cycles_payment_or_icp_payment': Variant.oftheMap({
                    cycles_or_icp_string + '_payment': Null()
                })
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}




Future<void> see_cycles_transfer_purchases(Nat page) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'see_cycles_transfer_purchases',
        caller: controller,
        put_bytes: c_forwards([
            page
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}



Future<void> see_cycles_bank_purchases(Nat page) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'see_cycles_bank_purchases',
        caller: controller,
        put_bytes: c_forwards([
            page
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

    Vector<Record> cbps = (cs[0] as Vector).cast_vector<Record>();

    for (Record cbp in cbps) {
        print('----------------');
        print('cycles_bank_principal: ${(cbp['cycles_bank_principal'] as PrincipalReference)}');
        print('cost_cycles: ${(cbp['cost_cycles'] as Nat)}');
        print('timestamp: ${(cbp['timestamp'] as Nat64)}');
    }
}









Future<void> call_canister_controller_see_new_canisters() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_see_new_canisters',
        caller: controller,
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}


Future<void> controller_put_new_canisters(Iterable<PrincipalReference> principals) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_put_new_canisters',
        caller: controller,
        put_bytes: c_forwards([Vector.oftheList<PrincipalReference>(principals.toList())])
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}



Future<void> controller_see_users_map_canisters() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_see_users_map_canisters',
        caller: controller,
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}



Future<void> controller_see_stable_size() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_see_stable_size',
        caller: controller,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}



Future<void> controller_see_new_canister_status(Principal new_canister_principal) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_see_new_canister_status',
        caller: controller,
        put_bytes: c_forwards([new_canister_principal.candid])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}


Future<void> controller_put_umc_code() async {
    Uint8List umc_code_module = File('../rust/target/wasm32-unknown-unknown/release/users_map_canister-o.wasm').readAsBytesSync();

    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_put_umc_code',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'module': Blob(umc_code_module),
                'module_hash': Blob(sha256.convert(umc_code_module).bytes),
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}


Future<void> controller_put_user_canister_code() async {
    Uint8List uc_code_module = File('../rust/target/wasm32-unknown-unknown/release/user_canister-o.wasm').readAsBytesSync();

    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_put_user_canister_code',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'module': Blob(uc_code_module),
                'module_hash': Blob(sha256.convert(uc_code_module).bytes),
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}



Future<void> controller_put_ctc_code() async {
    Uint8List ctc_code_module = File('../rust/target/wasm32-unknown-unknown/release/cycles_transferrer-o.wasm').readAsBytesSync();

    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_put_ctc_code',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'module': Blob(ctc_code_module),
                'module_hash': Blob(sha256.convert(ctc_code_module).bytes),
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}





Future<void> controller_deposit_cycles(Nat cycles, Principal topup_canister) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_deposit_cycles',
        caller: controller,
        put_bytes: c_forwards([
            cycles,
            topup_canister.candid
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}



Future<void> controller_see_metrics() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_see_metrics',
        caller: controller,        
    );
    // print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    Record metrics = cs[0] as Record;
    // print(metrics);
    print('------- METRICS -------');
    for (String metrics_field in [
        'global_allocator_counter',
        'stable_size',
        'cycles_balance',
        'new_canisters_count',
        'users_map_canisters_count',
        'cycles_transferrer_canisters_count',
        'user_canister_code_hash',
        'users_map_canister_code_hash',
        'cycles_transferrer_canister_code_hash',
        'latest_known_cmc_rate',
        'new_users_count',
        

    ]) {
        print('$metrics_field: ${metrics[metrics_field]}');
    }
}






Future<void> set_test_canisters_code(String mode) async {
    Uint8List install_code_arg = c_forwards([

    ]);
    Uint8List upgrade_code_arg = c_forwards([

    ]);
    
    await common.put_code_on_the_canister(
        controller,
        canister.principal,
        File('../gitpos/rust/target/wasm32-unknown-unknown/release/testcanister1-o.wasm').readAsBytesSync(),
        mode,
        ['install', 'reinstall'].contains(mode) ? install_code_arg : upgrade_code_arg
    );
    
    install_code_arg = c_forwards([

    ]);
    upgrade_code_arg = c_forwards([

    ]);
    
    await common.put_code_on_the_canister(
        controller,
        canister2.principal,
        File('../gitpos/rust/target/wasm32-unknown-unknown/release/testcanister2-o.wasm').readAsBytesSync(),
        mode,
        ['install', 'reinstall'].contains(mode) ? install_code_arg : upgrade_code_arg
    );
} 

Future<void> run_cycles_transfer_test_canisters() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'test_manual_reply',
        caller: controller,
        put_bytes: c_forwards([
            canister2.principal.candid,
            Nat(5)
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
    
}



Future<void> controller_cts_call_canister() async {
    /*
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': common.management.principal.candid,
                'method_name': Text('install_code'),
                'arg_raw': Blob(c_forwards([
                    Record.oftheMap({ 
                        'mode' : Variant.oftheMap({'upgrade': Null()}),
                        'canister_id': Principal(/*'mscqy-haaaa-aaaai-aahhq-cai'*/'woddh-aqaaa-aaaal-aazqq-cai').candid,
                        'wasm_module' : Blob(File('../rust/target/wasm32-unknown-unknown/release/user_canister-o.wasm').readAsBytesSync()),
                        'arg' : Blob(),
                    }),
                ])),
                'cycles': Nat(0)
            })
        ])
    );
    */
    /*
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': common.management.principal.candid,
                'method_name': Text('canister_status'),
                'arg_raw': Blob(c_forwards([
                    Record.oftheMap({ 
                        'canister_id': Principal('mscqy-haaaa-aaaai-aahhq-cai'/*'woddh-aqaaa-aaaal-aazqq-cai'*/).candid
                    }),
                ])),
                'cycles': Nat(0)
            })
        ])
    );
    */
    /*
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': Principal('mscqy-haaaa-aaaai-aahhq-cai').candid,
                'method_name': Text('cts_clear_user_canister_upgrade_fails'),
                'arg_raw': Blob(c_forwards([])),
                'cycles': Nat(0)
            })
        ])
    );
    */
    /*
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': Principal('mscqy-haaaa-aaaai-aahhq-cai').candid,
                'method_name': Text('cts_see_metrics'),
                'arg_raw': Blob(c_forwards([])),
                'cycles': Nat(0)
            })
        ])
    );
    */
    /*
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': Principal('mscqy-haaaa-aaaai-aahhq-cai').candid,
                'method_name': Text('cts_call_canister'),
                'arg_raw': Blob(c_forwards([
                    Record.oftheMap({
                        'callee': common.management.principal.candid,
                        'method_name': Text('install_code'),
                        'arg_raw': Blob(c_forwards([
                            Record.oftheMap({ 
                                'mode' : Variant.oftheMap({'upgrade': Null()}),
                                'canister_id': Principal(/*'mscqy-haaaa-aaaai-aahhq-cai'*/'woddh-aqaaa-aaaal-aazqq-cai').candid,
                                'wasm_module' : Blob(File('../rust/target/wasm32-unknown-unknown/release/user_canister-o.wasm').readAsBytesSync()),
                                'arg' : Blob(),
                            }),
                        ])),
                        'cycles': Nat(0)
                    })    
                ])),
                'cycles': Nat(0)
            })
        ])
    );
    */
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': canister.principal.candid,
                'method_name': Text('cycles_transfer'),
                'arg_raw': Blob(c_forwards([
                    Record.oftheMap({
                        'memo': Variant.oftheMap({
                            'Blob': Blob([...utf8.encode('UT'), controller3.principal.bytes.length, ...controller3.principal.bytes ])
                        })
                    })
                ])),
                'cycles': Nat(50000000000)
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
    if ((cs[0] as Variant).containsKey('Ok')) {
        print(c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes));
    }
    
}




Future<void> controller_see_new_users() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_see_new_users',
        caller: controller,
        put_bytes: c_forwards([])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}


Future<void> controller_download_state_snapshot_change_state_snapshot_and_put_state_snapshot() async {
    List<int> cts_data_candid_bytes = [];
    int chunk_i = 0;
    while (true) {
        Uint8List sponse = await canister.call(
            calltype: CallType.call,
            method_name: 'controller_download_state_snapshot',
            caller: controller,
            put_bytes: c_forwards([Nat32(chunk_i)])
        );
        //print(sponse);
        List<CandidType> cs = c_backwards(sponse);
        //print(cs);
        
        Option opt_chunk = cs[0] as Option;
        if (opt_chunk.value != null) {
            Uint8List chunk_bytes = (opt_chunk.value as Blob).bytes; 
            print('chunk: $chunk_i, len: ${chunk_bytes.length}');
            cts_data_candid_bytes.addAll(chunk_bytes);
            chunk_i += 1;
        } else {
            break;
        }
    }
    print(cts_data_candid_bytes.length);
    
    Record cts_data = c_backwards(Uint8List.fromList(cts_data_candid_bytes))[0] as Record;
    
    
    
    // change state snapshot
    //print(cts_data['create_new_users_map_canister_lock'] as Bool);
    //cts_data['create_new_users_map_canister_lock'] = Bool(false);
    //print(cts_data['create_new_users_map_canister_lock'] as Bool);
    
    

    Uint8List new_cts_data_candid_bytes = c_forwards([cts_data]);
    print('new_cts_data_candid_bytes length: ${new_cts_data_candid_bytes.length}');

    print(c_backwards(await canister.call(
        calltype: CallType.call,
        method_name: 'controller_clear_state_snapshot',
        caller: controller,
        put_bytes: c_forwards([])
    )));
    
    int upload_chunk_size = 1024*1024;
    for (int i = 0; i < new_cts_data_candid_bytes.length/upload_chunk_size; i++) {
        int chunk_start_i = i*upload_chunk_size;
        int chunk_end_i = i*upload_chunk_size+upload_chunk_size;
        if (chunk_end_i > new_cts_data_candid_bytes.length) {
            chunk_end_i = new_cts_data_candid_bytes.length;
        }
        print('chunk: $i, len: ${chunk_end_i - chunk_start_i}');
        Uint8List sponse = await canister.call(
            calltype: CallType.call,
            method_name: 'controller_append_state_snapshot_candid_bytes',
            caller: controller,
            put_bytes: c_forwards([Blob(new_cts_data_candid_bytes.sublist(chunk_start_i, chunk_end_i))])
        );
        //print(sponse);
        List<CandidType> cs = c_backwards(sponse);
        //print(cs);
        
        
    }
}



Future<void> controller_create_state_snapshot() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_create_state_snapshot',
        caller: controller,
        put_bytes: c_forwards([])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}



Future<void> controller_re_store_cts_data_out_of_the_state_snapshot() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_re_store_cts_data_out_of_the_state_snapshot',
        caller: controller,
        put_bytes: c_forwards([])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}



Future<void> controller_upgrade_umcs(Vector<PrincipalReference>? opt_umcs) async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_upgrade_umcs',
        caller: controller,
        put_bytes: c_forwards([
            Option(value: opt_umcs, value_type: Vector(isTypeStance: true, values_type: PrincipalReference(isTypeStance:true))),
            Blob([])
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}


Future<void> controller_upgrade_ucs_on_a_umc() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_upgrade_ucs_on_a_umc',
        caller: controller,
        put_bytes: c_forwards([
            Principal('mscqy-haaaa-aaaai-aahhq-cai').candid,
            Option(value:null, value_type: Vector(isTypeStance: true, values_type: PrincipalReference(isTypeStance:true))),
            Blob()
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}


Future<void> controller_put_uc_code_onto_the_umcs() async {
    Uint8List sponse = await canister.call(
        calltype: CallType.call,
        method_name: 'controller_put_uc_code_onto_the_umcs',
        caller: controller,
        put_bytes: c_forwards([
            Option(value:null, value_type: Vector(isTypeStance: true, values_type: PrincipalReference(isTypeStance:true))),
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}























Future<void> call_canister() async {
    late List<CandidType> cs;

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_size',

    // ));
    // print(cs);


    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_grow',
    //     put_bytes: c_forwards([
    //         Nat64(200),
    //     ])

    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_size',
    // ));
    // print(cs);


    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_write',
    //     put_bytes: c_forwards([
    //         Nat64(2),
    //         Blob([1,2,3]),
    //     ])

    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_size',
    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_bytes',
    // ));
    // Blob b = cs[0] as Blob;
    // // print(b.bytes);
    // print(b.length);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_read',
    //     put_bytes: c_forwards([
    //         Nat64(0),
    //         Nat64(20)
    //     ])

    // ));
    // print((cs[0] as Blob).bytes);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_clear_file_hashes',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);




    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'upload_frontcode_file_chunks',
    //     put_bytes: c_forwards([
    //         Text('/'),
    //         Record.oftheMap({
    //             'content_type': Text('text/plain; charset=utf-8'),
    //             'content_encoding': Text(''),
    //             'content': Blob(utf8.encode('Hello'))
    //         })
    //     ])
    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'upload_frontcode_file_chunks',
    //     put_bytes: c_forwards([
    //         Text('/sample.html'),
    //         Record.oftheMap({
    //             'content_type': Text('text/html; charset=utf-8'),
    //             'content_encoding': Text('gzip'),
    //             'content': Blob(gzip.encode(utf8.encode('<html><body><h1>Levi is Awesome.</h1></body></html>')))
    //         })
    //     ])
    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_get_file_hashes',
    // ));
    // // print(cs);
    // for (Record r in (cs[0] as Vector).cast_vector<Record>()) {
    //     print(r[0]);
    // }


    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'upload_frontcode_file_chunks',
    //     put_bytes: c_forwards([
    //         Text('/hello'),
    //         Blob(utf8.encode('hello'))
    //     ])
    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: CallType.call,
    //     method_name: 'public_get_file_hashes',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);

    cs = c_backwards(await canister.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'cycles_balance',
        put_bytes: c_forwards([])
    ));
    print(cs);

    // cs = c_backwards(await canister.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'sync_controllers',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'total_pages',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);

    cs = c_backwards(await canister.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'transfer_cycles',
        put_bytes: c_forwards([
            canister2.principal.candid,
            Nat64(3),
            Record.oftheMap({
                'memo': Variant.oftheMap({
                    'text': Text('memo')
                })
            })
        ])
    ));
    print(cs);

    // cs = c_backwards(await canister.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'total_pages',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);


    // cs = c_backwards(await canister.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'transfers',
    //     put_bytes: c_forwards([Nat64(1)])
    // ));
    // // print(cs);
    // Vector<Record> v = (cs[0] as Vector).cast_vector<Record>();
    // for (Record r in v) {
    //     print(r['cycles']);
    //     print(r['memo']);
    //     print(r['with']);
    //     if ( (r['in_or_out'] as Variant).containsKey('In') ) { print('In'); } 
    //     else if ( (r['in_or_out'] as Variant).containsKey('Out') ) { print('Out'); } 
    //     print(r['timestamp_nanos']);
    //     print('---------');

    // }
    
    
    cs = c_backwards(await canister.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'transfers',
        put_bytes: c_forwards([Nat64(1)])
    ));
    print(cs);
    Vector<Record> v = (cs[0] as Vector).cast_vector<Record>();
    for (Record r in v) {
        print(r['cycles']);
        print(r['memo']);
        print(r['with']);
        if ( (r['in_or_out'] as Variant).containsKey('In') ) { print('In'); } 
        else if ( (r['in_or_out'] as Variant).containsKey('Out') ) { print('Out'); } 
        print(r['timestamp_nanos']);
        print('---------');

    }

    cs = c_backwards(await canister2.call(
        caller: controller2,
        calltype: CallType.call,
        method_name: 'transfers',
        put_bytes: c_forwards([Nat64(1)])
    ));
    print(cs);
    Vector<Record> v2 = (cs[0] as Vector).cast_vector<Record>();
    for (Record r in v2) {
        print(r['cycles']);
        print(r['memo']);
        print(r['with']);
        if ( (r['in_or_out'] as Variant).containsKey('In') ) { print('In'); } 
        else if ( (r['in_or_out'] as Variant).containsKey('Out') ) { print('Out'); } 
        print(r['timestamp_nanos']);
        print('---------');

    }

    // cs = c_backwards(await canister.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'cycles_balance',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);


    cs = c_backwards(await canister2.call(
        caller: controller2,
        calltype: CallType.call,
        method_name: 'see_icp_xdr_conversion_rate',
        put_bytes: c_forwards([])
    ));
    print(cs);


}



Future<void> put_canister_controllers() async {


    List<CandidType> cs = c_backwards(await common.management.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'update_settings',
        put_bytes: c_forwards([
            Record.oftheMap({
                'canister_id': canister.principal.candid,
                'settings' : Record.oftheMap({
                    'controllers': Vector.oftheList<PrincipalReference>([
                        controller.principal.candid,
                        canister.principal.candid
                    ])
                })
            })
        ])
    ));
    print(cs);

    cs = c_backwards(await common.management.call(
        caller: controller2,
        calltype: CallType.call,
        method_name: 'update_settings',
        put_bytes: c_forwards([
            Record.oftheMap({
                'canister_id': canister2.principal.candid,
                'settings' : Record.oftheMap({
                    'controllers': Vector.oftheList<PrincipalReference>([
                        controller2.principal.candid,
                        canister2.principal.candid
                    ])
                })
            })
        ])
    ));
    print(cs);


}



Future<void> put_frontcode_build_web() async {
    Canister put_frontcode_on_the_canister = canister;
    print('putting frontcode on the canister: ${put_frontcode_on_the_canister.principal}');
    Directory build_web_dir = Directory('../frontcode/build/web/');
    await for (FileSystemEntity fse in build_web_dir.list(recursive: true, followLinks: false)) {
        print(fse.path);
        if ( await FileSystemEntity.isFile(fse.path) && !fse.path.contains('/canvaskit.wasm') ) {
            List<CandidType> cs = c_backwards(await put_frontcode_on_the_canister.call(
                calltype: CallType.call,
                method_name: 'controller_upload_frontcode_file_chunks',
                put_bytes: c_forwards([
                    Text( fse.path.contains('/index.html') ? '/' : fse.path.replaceFirst('../frontcode/build/web', '')),
                    Record.oftheMap({
                        'content_type': Text(''),
                        'content_encoding': Text('gzip'),
                        'content': Blob(gzip.encode(File(fse.path).readAsBytesSync()))
                    })
                ]),
                caller: controller
            ));
            print(cs);


        }
    }
}




Future<void> clear_frontcode_files() async {
    List<CandidType> cs = c_backwards(await canister.call(
        calltype: CallType.call,
        method_name: 'controller_clear_frontcode_files',
        caller: controller
    ));
    print(cs);
}




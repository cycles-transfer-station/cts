import 'dart:typed_data';
import 'dart:convert';
import 'dart:io';
import 'package:ic_tools/ic_tools.dart';
import 'package:ic_tools/common.dart' as common;
import 'package:ic_tools/common.dart' show IcpTokens, icp_id;
import 'package:ic_tools/candid.dart';
import 'package:ic_tools/tools.dart';
import 'package:crypto/crypto.dart';
import 'package:collection/collection.dart';


String home_dir = '/home/levi/Documents/code/cycles-transfer-station';


Map controllers_json = jsonDecode(utf8.decode(File('$home_dir/controllers_and_canisters.json').readAsBytesSync()));

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

Caller temp_icp_holder = CallerEd25519(
    public_key: Uint8List.fromList(controllers_json['temp_icp_holder']!['pub_key']!.cast<int>()),
    private_key: Uint8List.fromList(controllers_json['temp_icp_holder']!['s_key']!.cast<int>()),
);



late String dotc;

Map thp4z_controllers_json = jsonDecode(dotc);//utf8.decode(File('$home_dir/.c').readAsBytesSync()));

Caller thp4z_controller = CallerEd25519(
    public_key: Uint8List.fromList(thp4z_controllers_json['c']!['pub']!.cast<int>()),
    private_key: Uint8List.fromList(thp4z_controllers_json['c']!['-']!.cast<int>()),
);

Caller thp4z_controller2 = CallerEd25519(
    public_key: Uint8List.fromList(thp4z_controllers_json['c2']!['pub']!.cast<int>()),
    private_key: Uint8List.fromList(thp4z_controllers_json['c2']!['-']!.cast<int>()),
);

Caller thp4z_controller3 = CallerEd25519(
    public_key: Uint8List.fromList(thp4z_controllers_json['c3']!['pub']!.cast<int>()),
    private_key: Uint8List.fromList(thp4z_controllers_json['c3']!['-']!.cast<int>()),
);


 

final Canister cts = Canister(Principal('thp4z-laaaa-aaaam-qaaea-cai'));
final Canister cts_cycles_transferrer_1 = Canister(Principal('sok3f-uyaaa-aaaah-qanvq-cai'));
final Canister cts_cycles_transferrer_2 = Canister(Principal('ps5k4-oaaaa-aaaah-aao4a-cai'));
final Canister cts_cycles_transferrer_3 = Canister(Principal('a4ohg-niaaa-aaaaj-qacaq-cai'));
final Canister cycles_market = Canister(Principal('woddh-aqaaa-aaaal-aazqq-cai'));
final Canister cycles_market_cmcaller = Canister(Principal('hjgka-ziaaa-aaaam-qaeoa-cai'));



final Canister test_cts = Canister(Principal('bayhi-7yaaa-aaaai-qahca-cai'));
final Canister test_cts_cycles_transferrer_1 = Canister(Principal('ha4iv-6iaaa-aaaah-aapjq-cai'));
final Canister test_cycles_market = Canister(Principal('mscqy-haaaa-aaaai-aahhq-cai'));
final Canister test_cycles_market_cmcaller = Canister(Principal('sqotk-taaaa-aaaak-qaroa-cai'));




final Canister controller_cycles_bank = Canister(Principal('aaaaa-aa'));



Canister test_canister_1 = Canister(Principal('2fgzu-3iaaa-aaaai-qnh3a-cai'));
Canister test_canister_2 = Canister(Principal('2ch7a-wqaaa-aaaai-qnh3q-cai'));

Canister test_canister_3 = Canister(Principal('26dfr-bqaaa-aaaai-qnhzq-cai'));




Future<void> main(List<String> arguments) async {
    
    print('decrypt gpg? y/n');
    String? a = stdin.readLineSync();
    if (a == 'y') {
        await Future(()async{
            var process = await Process.start('gpg', ['-d', '$home_dir/.c.gpg']);
            process.stdout
                .transform(utf8.decoder)
                .forEach((String s) { 
                    if (s.contains('c') && s.contains('c2') && s.contains('c3')) {
                        dotc = s;
                    } else {
                        print(s);
                    }
                });   
            process.stderr
                .transform(utf8.decoder)
                .forEach(print);
            var exitCode = await process.exitCode;
            if (exitCode != 0) {
                throw Exception('gpg decrypt error: $exitCode');
            }
        });
    } else if (a != 'n') {
        throw Exception('must type y or n');
    }
    
    
    String first_command = arguments[0];

    if (first_command == 'create_controller') {
        await create_controller();
    }
    
    else if (first_command == 'canister_status') {
        await canister_status();
    }

    else if (first_command == 'top_up_canister') {
        await top_up_canister(arguments[1]);
    }

    else if (first_command == 'put_code_on_the_canister') {
        await put_code_on_the_canister(arguments[1]);
    }
    
    else if (first_command == 'uninstall_canister_code') {
        await uninstall_canister_code();
    }

    else if (first_command == 'create_canister') {
        await create_canister(arguments[1]);
    }

    else if (first_command == 'put_frontcode_files') {
        await put_frontcode_files(arguments[1]);
    }
    
    else if (first_command == 'clear_frontcode_files') {
        await clear_frontcode_files(arguments[1]);
    }
    
    else if (first_command == 'clear_frontcode_file') {
        await clear_frontcode_file(arguments[1]);
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






    else if (first_command == 'call_canister_topup_balance') {
        await call_canister_topup_balance();
    }
    
    else if (first_command == 'call_canister_convert_icp_balance_for_the_cycles_with_the_cmc_rate') {
        await call_canister_convert_icp_balance_for_the_cycles_with_the_cmc_rate(double.parse(arguments[1]));
    }
    

    else if (first_command == 'purchase_cycles_bank') {
        await purchase_cycles_bank(arguments[1]);
    }

    else if (first_command == 'see_cycles_transfer_purchases') {
        await see_cycles_transfer_purchases(Nat(BigInt.from(0)));
    }
    
    else if (first_command == 'see_cycles_bank_purchases') {
        await see_cycles_bank_purchases(Nat(BigInt.from(0)));
    }







    else if (first_command == 'call_canister_see_fees') {
        await call_canister_see_fees();
    }

    else if (first_command == 'call_canister_controller_see_new_canisters') {
        await call_canister_controller_see_new_canisters();
    }

    else if (first_command == 'controller_put_new_canisters') {
        await controller_put_new_canisters(arguments[1].split(',').map<PrincipalReference>((String pstring)=>Principal(pstring).candid));
    }

    else if (first_command == 'call_canister_controller_see_cycles_transferrer_canisters') {
        await call_canister_controller_see_cycles_transferrer_canisters();
    }
    
    else if (first_command == 'controller_put_cycles_transferrer_canisters') {
        await controller_put_cycles_transferrer_canisters();
    }

    else if (first_command == 'call_canister_controller_create_new_cycles_transferrer_canister') {
        await call_canister_controller_create_new_cycles_transferrer_canister();
    }

    else if (first_command == 'controller_see_cbsms') {
        await controller_see_cbsms();
    }

    else if (first_command == 'controller_see_stable_size') {
        await controller_see_stable_size();
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
    
    else if (first_command == 'cbsm_cts_see_user_canister_upgrade_fails') {
        await cbsm_cts_see_user_canister_upgrade_fails(Principal(arguments[1]));
    }
    
    else if (first_command == 'cbsm_cts_clear_user_canister_upgrade_fails') {
        await cbsm_cts_clear_user_canister_upgrade_fails(Principal(arguments[1]));
    }
    
    else if (first_command == 'cbsm_find_user') {
        await cbsm_find_user(cbsm: Principal(arguments[1]), user: Principal(arguments[2]));
    }
    
    else if (first_command == 'cbsm_cts_see_uc_code_module_hash') {
        await cbsm_cts_see_uc_code_module_hash(Principal(arguments[1]));
    }
    
    else if (first_command == 'cbsm_cts_see_metrics') {
        await cbsm_cts_see_metrics(Principal(arguments[1]));
    }

    else if (first_command == 'cycles_bank_cts_see_metrics') {
        await cycles_bank_cts_see_metrics(Principal(arguments[1]));
    }
    
    else if (first_command == 'controller_cts_call_canister_download_canister_data') {
        await controller_cts_call_canister_download_canister_data(Principal(arguments[1]));
    } 
    
    else if (first_command == 'controller_cts_call_canister_start_canister') {
        await controller_cts_call_canister_start_canister(Principal(arguments[1]));
    } 
    
    else if (first_command == 'controller_cts_call_canister_canister_status') {
        await controller_cts_call_canister_canister_status(Principal(arguments[1]));
    } 
    
    else if (first_command == 'controller_cts_call_cm_cts_see_do_payouts_errors') {
        await controller_cts_call_cm_cts_see_do_payouts_errors();
    }

    else if (first_command == 'controller_cts_call_cm_cts_clear_payouts_errors') {
        await controller_cts_call_cm_cts_clear_payouts_errors();
    }

    else if (first_command == 'controller_cts_call_cmcaller_cts_see_try_callbacks') {
        await controller_cts_call_cmcaller_cts_see_try_callbacks();
    }



    
    else if (first_command == 'cm_trigger_payouts') {
        await cm_trigger_payouts();
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
        await controller_upgrade_ucs_on_a_umc(Principal(arguments[1]));
    }

    else if (first_command == 'controller_put_uc_code_onto_the_umcs') {
        await controller_put_uc_code_onto_the_umcs();
    }

    else if (first_command == 'controller_upgrade_ctc') {
        await controller_upgrade_ctc(Principal(arguments[1]));
    }

    else if (first_command == 'controller_take_away_cycles_transferrer_canisters') {
        await controller_take_away_cycles_transferrer_canisters(arguments[1].split(',').map<Principal>((String ps)=>Principal(ps)).toList());
    }

    else if (first_command == 'controller_complete_users_purchase_cycles_bank') {
        List<Principal>? ps;
        if (arguments.length >= 2) {
            ps = arguments[1].split(',').map<Principal>((String ps)=>Principal(ps)).toList();
        } 
        await controller_complete_users_purchase_cycles_bank(ps);
    }

    
    else if (first_command == 'see_module_hashes') {
        await see_module_hashes();
    }


    












    else if (first_command == 'test') {
        // print(candid_text_hash('Err'));
        // print(candid_text_hash('cycles'));
        //print(candid_text_hash('Ok'));

        // print(c_forwards([Nat(BigInt.from(5))]));

        // print('cts: ${cts.principal.bytes}');
        
        //print(await common.transfer_icp(controller3, '9d67f85f54646c79a6e693b477bb0d2d161cb73e441bb885cebee568c2f4efc8', 1.0));
        //await common.top_up_canister(controller3, 3.0, cts.principal);
        
        /*
        for (Principal p in [
            controller.principal,
            controller2.principal,
            controller3.principal,
            cts.principal,
            cts.principal,
            canister3.principal,
            controller_cycles_bank.principal
        ]) {
            print(p);
            print(p.icp_id());
            print(await common.check_icp_balance(p.icp_id()));
        }
        */
        
        /*
        for (String t in [
            'Ok',
            'Err',
            'CheckIcpBalanceCallError',
            'CheckCurrentXdrPerMyriadPerIcpCmcRateError',
            'UserIcpLedgerBalanceTooLow',
            'membership_cost_icp',
            'user_icp_ledger_balance',
            'icp_ledger_transfer_fee',
            'NewUserIsInTheMiddleOfAnotherNewUserCall',
            'MaxNewUsers',
            'FoundUserCanister',
            'CreateUserCanisterCmcNotifyError',
            'MidCallError',    // re-try the call on this spo
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
            'running',
            'stopping',
            'stopped',
            'NoCyclesTransferrerCanistersFound',
            'FindUserInTheUsersMapCanistersError',
            'UserNotFound',
            'UsersMapCanistersFindUserCallFails',
            'CyclesTransferrerCanisterCodeNotFound'
        ]) {
            print(candid_text_hash(t));
        }
        */

        //print(sha256.convert(File('$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/users_map_canister-o.wasm').readAsBytesSync()).bytes);
        

        //print(await cts_main.controllers());

        //print(await test_canister_2.controllers());
        
        /*
        String ticph_icp_id = temp_icp_holder.principal.icp_id(); 
        print(ticph_icp_id);
        print(await common.check_icp_balance(ticph_icp_id));
        */
        
        //print(cts);
        //print(await cts.controllers());
        //print(controller);
        
        
        /*
        for (String t in [
            'icp_transfer',
            'cm_message_call_success_timestamp_nanos',
            'cm_message_callback_complete',
            'block_height',
            'timestamp_nanos',
            'cmcaller_cycles_payout_call_success_timestamp_nanos',
            'cmcaller_cycles_payout_callback_complete',
            'management_canister_posit_cycles_call_success',
            
        ]) {
            print('$t: ${candid_text_hash(t)}');
        }
        
        Vector<Record> cycles_positions_purchases = (c_backwards(await test_cycles_market.call(
            calltype: CallType.query,
            method_name: 'see_cycles_positions_purchases',
            put_bytes: c_forwards([Record.oftheMap({ 'chunk_i': Nat(BigInt.from(0)) })])
        ))[0] as Option).cast_option<Vector>().value!.cast_vector<Record>();
        
        for (Record cppr in cycles_positions_purchases) {
            print('id: ${(cppr['id'] as Nat).value}');
            print('icp_payout_lock: ${(cppr['icp_payout_lock'] as Bool).value}');
            print('cycles_payout_lock: ${(cppr['cycles_payout_lock'] as Bool).value}');
            print('icp_payout_data: ${(cppr['icp_payout_data'] as Record)}');
            print('cycles_payout_data: ${(cppr['cycles_payout_data'] as Record)}');
        }
        
        */
        
        //print(await test_canister_1.controllers());
        
        /*
        print(candid_text_hash('cycles'));
        
        print(c_backwards(await test_canister_1.call(
            caller: controller,
            calltype: CallType.call,
            method_name: 'download_cm_cycles_positions',
            put_bytes: c_forwards([
                Nat(BigInt.from(0))
                /*Record.oftheMap({
                    'cycles_position_positor' : test_canister_1.principal,
                    'cycles_position_xdr_permyriad_per_icp_rate' : Nat64(BigInt.from(23500)),
                    'cycles_market_purchase_cycles_position_quest' : Record.oftheMap({
                        'cycles_position_id': Nat(BigInt.from(2)),
                        'cycles': Nat(BigInt.from(800000019000)),
                    }),
                })
                */
            ])
        )));
        */
        
        //print(common.icp_id(cycles_market.principal, subaccount_bytes: principal_as_an_icpsubaccountbytes(test_canister_1.principal)));
        //print(common.icp_id(cycles_market.principal, subaccount_bytes: principal_as_an_icpsubaccountbytes(test_canister_2.principal)));
        
        //print(Directory.current);
        
        
        Uint8List sponse = await cts.call(
            calltype: CallType.call,
            method_name: 'controller_call_canister',
            caller: thp4z_controller,
            put_bytes: c_forwards([
                Record.oftheMap({
                    'callee': common.ledger.principal,
                    'method_name': Text('transfer'),
                    'arg_raw': Blob(
                        c_forwards([
                            Record.oftheMap({
                                'to': Blob(hexstringasthebytes('fe8d1cbdd9b501418d6cdb747d0bc89bc05ec49cf19d5f44d2e2d819cf23ca78')),
                                'fee': IcpTokens.oftheDoubleString('0.0001'),
                                'memo': Nat64(BigInt.from(1)),
                                'from_subaccount': Option<Vector<Nat8>>(value: null, value_type: Vector(values_type: Nat8(), isTypeStance:true)),
                                'created_at_time': Option<Record>(value:null, value_type: Record.oftheMap({'timestamp_nanos': Nat64()}, isTypeStance:true)),
                                'amount': IcpTokens.oftheDoubleString('1.2')
                            })
                        ])
                    ),
                    'cycles': Nat(BigInt.from(0))
                })
            ])
        );
        
        print(icp_id(cts.principal));
        print(await common.check_icp_balance(icp_id(cts.principal)));
        
        
        
        
    }



    else {
        throw Exception('"$first_command" is not a known command.');
    }

}


Uint8List principal_as_an_icpsubaccountbytes(Principal principal) {
    List<int> bytes = []; // an icp subaccount is 32 bytes
    bytes.add(principal.bytes.length);
    bytes.addAll(principal.bytes);
    while (bytes.length < 32) { bytes.add(0); }
    return Uint8List.fromList(bytes);
}



Future<void> see_module_hashes() async {
    print('cts: ${cts}');
    //print('cts-controllers: ${await cts.controllers()}');
    print('cts-module_hash: ${await cts.module_hash()}');
    print('\ncts_cycles_transferrer_1: ${cts_cycles_transferrer_1}\n\tmodule_hash ${await cts_cycles_transferrer_1.module_hash()}');
    print('\ncts_cycles_transferrer_2: ${cts_cycles_transferrer_2}\n\tmodule_hash ${await cts_cycles_transferrer_2.module_hash()}');
    print('\ncts_cycles_transferrer_3: ${cts_cycles_transferrer_3}\n\tmodule_hash ${await cts_cycles_transferrer_3.module_hash()}');
    print('\ncycles_market: ${cycles_market}\n\tmodule_hash ${await cycles_market.module_hash()}');
    print('\ncycles_market_cmcaller: ${cycles_market_cmcaller}\n\tmodule_hash ${await cycles_market_cmcaller.module_hash()}');        
    
    print('\ntest_cts: ${test_cts}');
//        print('test_cts-controllers: ${await test_cts.controllers()}');
    print('\ntest_cts-module_hash: ${await test_cts.module_hash()}');
    print('\ntest_cts_cycles_transferrer_1: ${test_cts_cycles_transferrer_1}\n\tmodule_hash ${await test_cts_cycles_transferrer_1.module_hash()}');
    print('\ntest_cycles_market: ${test_cycles_market}\n\tmodule_hash ${await test_cycles_market.module_hash()}');
    print('\ntest_cycles_market_cmcaller: ${test_cycles_market_cmcaller}\n\tmodule_hash ${await test_cycles_market_cmcaller.module_hash()}');        
    
    Principal test_cbsm = Principal('34oen-caaaa-aaaai-qntlq-cai');
    print('\ntest_cbsm: ${test_cbsm}');
    print('\ntest_cbsm wasm_hash: ${await Canister(test_cbsm).module_hash()}');
 
    Principal test_cycles_bank = Principal('mq76c-siaaa-aaaao-aarvq-cai');
    print('\ntest_cycles_bank: ${test_cycles_bank}\nmodule_hash: ${await Canister(test_cycles_bank).module_hash()}');
    
    Principal test_cycles_bank_2 = Principal('lwwnf-oiaaa-aaaal-qbhea-cai');
    print('\ntest_cycles_bank_2: ${test_cycles_bank_2}\nmodule_hash: ${await Canister(test_cycles_bank_2).module_hash()}');
   
    
    List<String> module_paths = [
        '$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cts.wasm',
        '$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cbs_map.wasm',
        '$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cycles_bank.wasm',
        '$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cycles_market.wasm',
        '$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cycles_transferrer.wasm',
        '$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cm_caller.wasm',
    ];
    for (String path in module_paths) {
        print('\n${path.replaceFirst(home_dir+'/gitpos/rust/target/wasm32-unknown-unknown/release/', '')} : module_hash: ${sha256.convert(File(path).readAsBytesSync()).bytes}');
    }

}




Future<void> create_controller() async {
    CallerEd25519 controller = CallerEd25519.new_keys();
    
    
    print(icp_id(controller.principal));
    print('pub: ${controller.public_key}');
    print('priv: ${controller.private_key}');
    
}



Future<void> create_canister(String create_icp) async {
    print(await common.check_icp_balance(icp_id(controller.principal)));
    Principal can_id = await common.create_canister(controller, IcpTokens.oftheDoubleString(create_icp));
    print(can_id);
    Canister can = Canister(can_id);
    print(await can.controllers());
    print(await common.check_icp_balance(icp_id(controller.principal)));
}

Future<void> canister_status() async {
    Principal canister_id = cts.principal;
    print('canister: ${canister_id.text}');
    print(await common.check_canister_status(thp4z_controller, canister_id));
}

Future<void> top_up_canister(String top_up_icp) async {
    print(await common.check_icp_balance(icp_id(controller.principal)));
    await common.top_up_canister(controller, IcpTokens.oftheDoubleString(top_up_icp), cts.principal);
    print(await common.check_icp_balance(icp_id(controller.principal)));

}

Future<void> put_code_on_the_canister(String mode) async {

    Principal put_code_on_the_canister_id = test_cycles_market.principal;
    Uint8List wasm_module = File('$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cycles_market.wasm').readAsBytesSync();
    Caller use_controller = controller;
    
    Uint8List install_code_arg = c_forwards([
        Record.oftheMap({
            /*            
            'controllers': Vector.oftheList<PrincipalReference>([
                thp4z_controller.principal, 
                thp4z_controller2.principal, 
                thp4z_controller3.principal
            ]),
            'cycles_market_id': cycles_market.principal,
            'cycles_market_cmcaller': cycles_market_cmcaller.principal
            */
            /*
            'cts_id': cts.principal,
            'cm_caller': cycles_market_cmcaller.principal,
            */
            //'cts_id': cts.principal
            
            /*
            'cycles_market_id': cycles_market.principal,
            'cts_id': cts.principal
            */
            
            'user_id': controller.principal,
            'cts_id': cts.principal,
            'cbsm_id': cts.principal, 
            'cycles_market_id': cycles_market.principal, 
            'cycles_market_cmcaller': cycles_market_cmcaller.principal,
            'storage_size_mib': Nat(BigInt.from(20)),                         
            'lifetime_termination_timestamp_seconds': Nat(BigInt.from(1697259715)),
            'cycles_transferrer_canisters': Vector.oftheList([
                cts_cycles_transferrer_1.principal,
                cts_cycles_transferrer_2.principal,
                cts_cycles_transferrer_3.principal,
            ])
        
        })
    ]);
    Uint8List upgrade_code_arg = c_forwards([
        
    ]);
    
    print('you are about to $mode code onto the canister: ${put_code_on_the_canister_id}\ntype: "$mode" to continue.');
    if (stdin.readLineSync() != mode) {
        throw Exception('$mode confirmation fail.');
    }


    print('stop_canister');
    Uint8List stop_canister_sponse = await common.management.call(
        calltype: CallType.call,
        method_name: 'stop_canister',
        caller: use_controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'canister_id': put_code_on_the_canister_id
            })
        ])
    );
    print(stop_canister_sponse);
    List<CandidType> stop_canister_cs = c_backwards(stop_canister_sponse);
    print(stop_canister_cs);

    print(mode + ' code');
    
    await common.put_code_on_the_canister(
        use_controller,
        put_code_on_the_canister_id,
        wasm_module,
        mode,
        ['install', 'reinstall'].contains(mode) ? install_code_arg : upgrade_code_arg
    );

    
    print('start_canister');
    Uint8List start_canister_sponse = await common.management.call(
        calltype: CallType.call,
        method_name: 'start_canister',
        caller: use_controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'canister_id': put_code_on_the_canister_id
            })
        ])
    );
    print(start_canister_sponse);
    List<CandidType> start_canister_cs = c_backwards(start_canister_sponse);
    print(start_canister_cs);
   
}


Future<void> uninstall_canister_code() async {
    
    Principal uninstall_canister_id = cts_cycles_transferrer_1.principal; 
    
    print('you are about to uninstall_code onto the canister: ${uninstall_canister_id}\ntype: "confirm" to continue.');
    if (stdin.readLineSync() != 'confirm') {
        throw Exception('confirmation fail.');
    } 

    Uint8List sponse = await common.management.call(
        calltype: CallType.call,
        method_name: 'uninstall_code',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'canister_id': uninstall_canister_id
            })
        ])
    );
    print(sponse);
    print(c_backwards(sponse));
}



Future<void> change_canister_settings() async {
    Uint8List change_canister_settings_sponse = await common.management.call(
        caller: thp4z_controller,
        calltype: CallType.call,
        method_name: 'update_settings',
        put_bytes: c_forwards([
            Record.oftheMap({
                'canister_id': cts.principal,
                'settings': Record.oftheMap({
                    // freezing_threshold : opt nat,
                    'controllers' : Option(value: Vector.oftheList<PrincipalReference>([
                        cts.principal,
                        thp4z_controller.principal,
                        thp4z_controller2.principal,
                        thp4z_controller3.principal,
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
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'new_user',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);

}



Future<void> call_canister_find_user_canister() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'find_user_canister',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);

}



Future<void> call_user_canister_user_cycles_balance() async {
    Uint8List sponse = await controller_cycles_bank.call(
        calltype: CallType.call,
        method_name: 'user_cycles_balance',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}


Future<void> call_user_canister_user_icp_balance() async {
    Uint8List sponse = await controller_cycles_bank.call(
        calltype: CallType.call,
        method_name: 'user_icp_balance',
        caller: controller3,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}


Future<void> call_user_canister_user_download_cycles_transfers_in(int chunk_i) async {
    Uint8List sponse = await controller_cycles_bank.call(
        calltype: CallType.call,
        method_name: 'user_download_cycles_transfers_in',
        caller: controller3,
        put_bytes: c_forwards([Nat32(chunk_i)])
    );
    //print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    //print(cs);
    Option opt_l = (cs[0] as Option);
    if (opt_l.value != null) {
        Vector<Record> l = (opt_l.value as Vector).cast_vector<Record>();
        if (l != null) {
            for (Record cycles_transfer_in in l) {
                print('-----');
                for (String field_name in [
                    'timestamp_nanos',
                    'canister',
                    'cycles'
                ]) {
                    print('${field_name}: ${cycles_transfer_in[field_name]}');
                }
            }
        }
    }
}

Future<void> call_user_canister_user_transfer_cycles() async {

    Uint8List sponse = await controller_cycles_bank.call(
        calltype: CallType.call,
        method_name: 'user_transfer_cycles',
        caller: controller3,
        put_bytes: c_forwards([
            // sending cycles for my self, out then in
            Record.oftheMap({
                'cycles': Nat(BigInt.from(3)),
                'canister_id': cts.principal.candid,
                'cycles_transfer_memo': Variant.oftheMap({
                    'Blob': Blob([])
                })
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    print(cs);
}

Future<void> call_user_canister_user_download_cycles_transfers_out(int chunk_i) async {
    Uint8List sponse = await controller_cycles_bank.call(
        calltype: CallType.call,
        method_name: 'user_download_cycles_transfers_out',
        caller: controller3,
        put_bytes: c_forwards([Nat32(chunk_i)])
    );
    //print(sponse);
    List<CandidType> cs = c_backwards(sponse); 
    //print(cs);
    Option opt_l = (cs[0] as Option);
    if (opt_l.value != null) {
        Vector<Record> l = (opt_l.value as Vector).cast_vector<Record>();
        if (l != null) {   
            for (Record cycles_transfer_out in l) {
                print('-----');
                for (String field_name in [
                    'timestamp_nanos',
                    'canister_id',
                    'cycles_transfer_memo',
                    'cycles_sent',
                    'cycles_accepted',
                    'call_error',
                    'fee_paid'
                ]) {
                    print('${field_name}: ${cycles_transfer_out[field_name]}');
                }
            }
        }
    }
}



























Future<void> call_canister_topup_balance() async {
    Uint8List sponse = await cts.call(
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
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'convert_icp_balance_for_the_cycles_with_the_cmc_rate',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'icp': Record.oftheMap({
                    'e8s': Nat64(BigInt.from((icp * 100000000).toInt()))
                })
            })
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}





Future<void> call_canister_collect_balance(Uint8List params) async {
    Uint8List sponse = await cts.call(
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
    
    Uint8List sponse = await cts.call(
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
    Uint8List sponse = await cts.call(
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
    Uint8List sponse = await cts.call(
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



















Future<void> call_canister_see_fees() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'see_fees',    
    );
    // print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    Record fees = cs[0] as Record;
    print('------- FEES -------');
    for (String field_name in [
        'purchase_cycles_bank_cost_cycles',
        'purchase_cycles_bank_upgrade_cost_cycles',
        'purchase_cycles_transfer_cost_cycles',
        'convert_icp_for_the_cycles_with_the_cmc_rate_cost_cycles',
        'minimum_cycles_transfer_into_user',
        'cycles_transfer_into_user_user_not_found_fee_cycles',
        'cycles_per_user_per_103_mib_per_year',        

    ]) {
        print('$field_name: ${fees[field_name]}');
    }
}







Future<void> call_canister_controller_see_new_canisters() async {
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_see_canisters_for_the_use',
        caller: controller,
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}


Future<void> controller_put_new_canisters(Iterable<PrincipalReference> principals) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_put_new_canisters',
        caller: controller,
        put_bytes: c_forwards([Vector.oftheList<PrincipalReference>(principals.toList())])
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}



Future<void> controller_put_cycles_transferrer_canisters() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_put_cycles_transferrer_canisters',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            Vector.oftheList<PrincipalReference>([
                cts_cycles_transferrer_1.principal,
                cts_cycles_transferrer_2.principal,
                cts_cycles_transferrer_3.principal,
            ])
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
} 



Future<void> call_canister_controller_see_cycles_transferrer_canisters() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_see_cycles_transferrer_canisters',
        caller: thp4z_controller,
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}


Future<void> call_canister_controller_create_new_cycles_transferrer_canister() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_create_new_cycles_transferrer_canister',
        caller: controller,
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}



Future<void> controller_see_cbsms() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_see_cbsms',
        caller: thp4z_controller,
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs); 
}



Future<void> controller_see_stable_size() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_see_stable_size',
        caller: controller,
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
}




Future<void> controller_put_umc_code() async {
    Uint8List umc_code_module = File('$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cbs_map.wasm').readAsBytesSync();

    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_put_umc_code',
        caller: thp4z_controller,
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
    Uint8List uc_code_module = File('$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cycles_bank.wasm').readAsBytesSync();
    print(sha256.convert(uc_code_module).bytes);
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_put_cycles_bank_canister_code',
        caller: thp4z_controller,
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
    Uint8List ctc_code_module = File('$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cycles_transferrer-o.wasm').readAsBytesSync();

    Uint8List sponse = await cts.call(
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
    Uint8List sponse = await cts.call(
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


Future<void> controller_complete_users_purchase_cycles_bank(List<Principal>? ps) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_complete_users_purchase_cycles_bank',
        caller: controller,
        put_bytes: c_forwards([
            Option(value: ps != null ? Vector.oftheList(ps) : null, value_type: Vector(values_type: PrincipalReference(isTypeStance: true), isTypeStance: true))
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}








Future<void> controller_see_metrics() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_see_metrics',
        caller: thp4z_controller,        
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
        'canisters_for_the_use_count',
        'cbsms_count',
        'cycles_transferrer_canisters_count',
        'cycles_bank_canister_code_hash',
        'cbsm_code_hash',
        'cycles_transferrer_canister_code_hash',
        'latest_known_cmc_rate',
        'users_purchase_cycles_bank_count',
        'users_burn_icp_mint_cycles_count',
        'users_transfer_icp_count',
    ]) {
        print('$metrics_field: ${metrics[metrics_field]}');
    }
}






Future<void> set_test_canisters_code(String mode) async {
    /*
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': common.management.principal.candid,
                'method_name': Text('update_settings'),
                'arg_raw': Blob(c_forwards([
                    Record.oftheMap({ 
                        'canister_id': test_canister_1.principal.candid,//Principal('woddh-aqaaa-aaaal-aazqq-cai'/*'woddh-aqaaa-aaaal-aazqq-cai', 2ch7a-wqaaa-aaaai-qnh3q-cai*/).candid,
                        'settings': Record.oftheMap({
                            'controllers': Vector.oftheList([controller.principal.candid, cts.principal.candid])
                        })
                    
                    }),
                ])),
                'cycles': Nat(0)
            })
        ])
    );
    sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': common.management.principal.candid,
                'method_name': Text('update_settings'),
                'arg_raw': Blob(c_forwards([
                    Record.oftheMap({ 
                        'canister_id': test_canister_2.principal.candid,//Principal('woddh-aqaaa-aaaal-aazqq-cai'/*'woddh-aqaaa-aaaal-aazqq-cai', 2ch7a-wqaaa-aaaai-qnh3q-cai*/).candid,
                        'settings': Record.oftheMap({
                            'controllers': Vector.oftheList([controller.principal.candid, cts.principal.candid])
                        })
                    
                    }),
                ])),
                'cycles': Nat(0)
            })
        ])
    );
    */
    Uint8List install_code_arg = c_forwards([

    ]);
    Uint8List upgrade_code_arg = c_forwards([

    ]);
    
    await common.put_code_on_the_canister(
        controller,
        test_canister_1.principal,
        File('../../testcanister1/target/wasm32-unknown-unknown/release/testcanister1-o.wasm').readAsBytesSync(),
        mode,
        ['install', 'reinstall'].contains(mode) ? install_code_arg : upgrade_code_arg
    );
    
    install_code_arg = c_forwards([

    ]);
    upgrade_code_arg = c_forwards([

    ]);
    
    await common.put_code_on_the_canister(
        controller,
        test_canister_2.principal,
        File('../../testcanister2/target/wasm32-unknown-unknown/release/testcanister2-o.wasm').readAsBytesSync(),
        mode,
        ['install', 'reinstall'].contains(mode) ? install_code_arg : upgrade_code_arg
    );
} 

Future<void> run_cycles_transfer_test_canisters() async {
    Uint8List sponse = await test_canister_1.call(
        calltype: CallType.call,
        method_name: 'test_cycles_cept_then_trap',
        caller: controller,
        put_bytes: c_forwards([
            test_canister_2.principal.candid
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
    
}



Future<void> controller_cts_call_canister() async {
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': common.management.principal,
                'method_name': Text('deposit_cycles'),
                'arg_raw': Blob(
                    c_forwards([
                        Record.oftheMap({
                            'canister_id': cts.principal,
                            
                            /*
                            'canister_id': test_canister_3.principal,
                            'settings': Record.oftheMap({
                                'controllers' : Option(value: Vector.oftheList<PrincipalReference>([
                                    test_cts.principal,
                                    controller.principal,
                                    controller2.principal,
                                    controller3.principal,
                                ])),
                            })
                            */
                            /*
                            'canister_id': Principal('mq76c-siaaa-aaaao-aarvq-cai'),
                            'mode' : Variant.oftheMap({'install': Null()}),
                            'wasm_module' : Blob(File('$home_dir/gitpos/rust/target/wasm32-unknown-unknown/release/cycles_bank.wasm').readAsBytesSync()),
                            'arg' : Blob(c_forwards([
                                Record.oftheMap({
                                    'user_id': Principal('l6nkv-qht26-p4eeg-fp3xv-nfo4y-4yxeh-gxmp3-y7p3a-y7eks-dlihm-xae'),
                                    'cts_id': Principal('bayhi-7yaaa-aaaai-qahca-cai'),
                                    'cbsm_id': Principal('34oen-caaaa-aaaai-qntlq-cai'), 
                                    'cycles_market_id': test_cycles_market.principal, 
                                    'cycles_market_cmcaller': test_cycles_market_cmcaller.principal,
                                    'storage_size_mib': Nat(BigInt.from(50)),                         
                                    'lifetime_termination_timestamp_seconds': Nat(BigInt.from(1699117580)),
                                    'cycles_transferrer_canisters': Vector.oftheList([test_cts_cycles_transferrer_1.principal])
                                })
                            ])),
                            */
                        })
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
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


Future<void> cbsm_cts_see_user_canister_upgrade_fails(Principal cbsm) async {   
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': cbsm,
                'method_name': Text('cts_see_user_canister_upgrade_fails'),
                'arg_raw': Blob(
                    c_forwards([
                        Nat64(BigInt.from(0))
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
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

Future<void> cbsm_cts_clear_user_canister_upgrade_fails(Principal cbsm) async {   
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': cbsm,
                'method_name': Text('cts_clear_user_canister_upgrade_fails'),
                'arg_raw': Blob(
                    c_forwards([
                        
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
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




Future<void> cbsm_find_user({ required Principal cbsm, required Principal user}) async {   
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': cbsm,
                'method_name': Text('find_user'),
                'arg_raw': Blob(
                    c_forwards([
                        user
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
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


Future<void> cbsm_cts_see_uc_code_module_hash(Principal cbsm) async {
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': cbsm,
                'method_name': Text('cts_see_uc_code_module_hash'),
                'arg_raw': Blob(
                    c_forwards([
                        
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
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

Future<void> cbsm_cts_see_metrics(Principal cbsm) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': cbsm,
                'method_name': Text('cts_see_metrics'),
                'arg_raw': Blob(
                    c_forwards([
                        
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
            })
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
    if ((cs[0] as Variant).containsKey('Ok')) {
        Record metrics = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes)[0] as Record; 
        print(metrics);
        for (String f in [
            'global_allocator_counter',    
            'stable_size',
            'cycles_balance',
            'user_canister_code_hash',
            'users_map_len',
            'user_canister_upgrade_fails_len',
        ]) {
            print('$f: ${metrics[f]}');        
        }
    }
}

Future<void> cycles_bank_cts_see_metrics(Principal cycles_bank) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': cycles_bank,
                'method_name': Text('cts_see_metrics'),
                'arg_raw': Blob(
                    c_forwards([
                        
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
            })
        ])
    );
    //print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    //print(cs);
    if ((cs[0] as Variant).containsKey('Ok')) {
        Record metrics = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes)[0] as Record; 
        //print(metrics);
        for (String f in [
            'canister_cycles_balance',
            'cycles_balance',
            'ctsfuel_balance',
            'wasm_memory_size_bytes',
            'stable_memory_size_bytes',
            'storage_size_mib',
            'lifetime_termination_timestamp_seconds',
            'cycles_transferrer_canisters',
            'user_id',
            'user_canister_creation_timestamp_nanos',
            'cycles_transfers_id_counter',
            'cycles_transfers_out_len',
            'cycles_transfers_in_len',
            'memory_size_at_the_start',
            'storage_usage',
            'free_storage',
        ]) {
            print('$f: ${metrics[f]}');        
        }
    }
}


Future<void> controller_cts_call_canister_download_canister_data(Principal canister) async {
    List<int> canister_data = [];
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': canister,
                'method_name': Text('cts_create_state_snapshot'),
                'arg_raw': Blob(
                    c_forwards([
                        
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
            })
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    //print(cs);
    List<CandidType> canister_sponse_cs = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes);
    final int canister_data_len = (canister_sponse_cs[0] as Nat64).value.toInt();
    
    for(int i = 0; true; i=i+1) {
        Uint8List sponse = await cts.call(
            calltype: CallType.call,
            method_name: 'controller_call_canister',
            caller: thp4z_controller,
            put_bytes: c_forwards([
                Record.oftheMap({
                    'callee': canister,
                    'method_name': Text('cts_download_state_snapshot'),
                    'arg_raw': Blob(
                        c_forwards([
                            Nat(BigInt.from(i))
                        ])
                    ),
                    'cycles': Nat(BigInt.from(0))
                })
            ])
        );
        List<CandidType> cs = c_backwards(sponse);
        //print(cs);
        List<CandidType> canister_sponse_cs = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes);
        if ((canister_sponse_cs[0] as Option).value == null) {
            break;
        } else {
            //print(canister_sponse_cs);
            canister_data.addAll(Blob.oftheVector(((canister_sponse_cs[0] as Option).value as Vector).cast_vector<Nat8>()).bytes);
        }
    }
    if (canister_data.length != canister_data_len) {
        throw Exception('wrong length of the data download');
    }
    print('len: ${canister_data.length}');
    /*
    print(
"""final List<int> data = ${canister_data};"""
    );
    */
    print('\n');
    print((c_backwards(Uint8List.fromList(canister_data))[0] as Record)['users_map']);
    
}


Future<void> controller_cts_call_canister_start_canister(Principal canister) async {
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': common.management.principal,
                'method_name': Text('start_canister'),
                'arg_raw': Blob(
                    c_forwards([
                        Record.oftheMap({
                            'canister_id': canister
                        }) 
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
            })
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
    List<CandidType> canister_sponse_cs = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes);
    print(canister_sponse_cs);
}


Future<void> controller_cts_call_canister_canister_status(Principal canister) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': common.management.principal,
                'method_name': Text('canister_status'),
                'arg_raw': Blob(
                    c_forwards([
                        Record.oftheMap({
                            'canister_id': canister
                        }) 
                    ])
                ),
                'cycles': Nat(BigInt.from(0))
            })
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    print(cs);
    List<CandidType> canister_sponse_cs = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes);
    print(canister_sponse_cs);
    
    Record status_record = (canister_sponse_cs[0] as Record);
    
    for(String f in [
        'module_hash',
        'memory_size',
        'cycles',
        'idle_cycles_burned_per_day',
    ]) {
        print('$f: ${status_record[f]}');
    }
    
    late String status;
    for (String s in ['running', 'stopping', 'stopped']) {
        if (candid_text_hash(s) == (status_record['status'] as Variant).keys.first) {
            status = s;
        }
    }
    print('status: ${status}');
    
    Record settings = status_record['settings'] as Record;
    for (String f in [
        'controllers', 
        'compute_allocation',
        'memory_allocation',
        'freezing_threshold'
    ]) {
        print('$f: ${settings[f]}');
    }
    
}


Future<void> controller_cts_call_cm_cts_see_do_payouts_errors() async {
    List<Record> do_payouts_errors = []; 
    
    int i = 0;
    while (true) {
        Uint8List sponse = await test_cts.call(
            calltype: CallType.call,
            method_name: 'controller_call_canister',
            caller: controller,
            put_bytes: c_forwards([
                Record.oftheMap({
                    'callee': test_cycles_market.principal,
                    'method_name': Text('cts_see_payouts_errors'),
                    'arg_raw': Blob(
                        c_forwards([
                            Nat32(i)
                        ])
                    ),
                    'cycles': Nat(BigInt.from(0))
                })
            ])
        );
        List<CandidType> cs = c_backwards(sponse);
        //print(cs);
        List<CandidType> canister_sponse_cs = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes);
        //print(canister_sponse_cs);
        
        Option<Vector> opt_logs = (canister_sponse_cs[0] as Option).cast_option<Vector>();
        if (opt_logs.value == null) {
            break;
        } else {
            do_payouts_errors.addAll(opt_logs.value!.cast_vector<Record>());    
        }
        
        i = i+1;
    }
    
    
    print(do_payouts_errors);
    
}


Future<void> controller_cts_call_cm_cts_clear_payouts_errors() async {
    
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': test_cycles_market.principal,
                'method_name': Text('cts_clear_payouts_errors'),
                'arg_raw': Blob(
                    c_forwards([])
                ),
                'cycles': Nat(BigInt.from(0))
            })
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    //print(cs);
    List<CandidType> canister_sponse_cs = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes);
    print(canister_sponse_cs);
    
}


Future<void> controller_cts_call_cmcaller_cts_see_try_callbacks() async {
    
    Uint8List sponse = await test_cts.call(
        calltype: CallType.call,
        method_name: 'controller_call_canister',
        caller: controller,
        put_bytes: c_forwards([
            Record.oftheMap({
                'callee': test_cycles_market_cmcaller.principal,
                'method_name': Text('cts_see_try_callbacks'),
                'arg_raw': Blob(
                    c_forwards([])
                ),
                'cycles': Nat(BigInt.from(0))
            })
        ])
    );
    List<CandidType> cs = c_backwards(sponse);
    //print(cs);
    List<CandidType> canister_sponse_cs = c_backwards(((cs[0] as Variant)['Ok'] as Blob).bytes);
    print(canister_sponse_cs);
    
}





Future<void> cm_trigger_payouts() async {
    print(c_backwards(await test_cycles_market.call(
        caller: controller,
        method_name: 'trigger_payouts',
        calltype: CallType.call
    )));
}







Future<void> controller_see_new_users() async {
    Uint8List sponse = await cts.call(
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
        Uint8List sponse = await cts.call(
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

    print(c_backwards(await cts.call(
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
        Uint8List sponse = await cts.call(
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
    Uint8List sponse = await cts.call(
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
    Uint8List sponse = await cts.call(
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
    Uint8List sponse = await test_cts.call(
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


Future<void> controller_upgrade_ucs_on_a_umc(Principal umc) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_upgrade_ucs_on_a_umc',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            umc,
            Option<Vector<PrincipalReference>>(value:null, value_type: Vector(isTypeStance: true, values_type: PrincipalReference(isTypeStance:true))),
            Blob([])
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}


Future<void> controller_put_uc_code_onto_the_umcs() async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_put_uc_code_onto_the_umcs',
        caller: thp4z_controller,
        put_bytes: c_forwards([
            Option<Vector<PrincipalReference>>(value:null, value_type: Vector(isTypeStance: true, values_type: PrincipalReference(isTypeStance:true))),
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}


Future<void> controller_upgrade_ctc(Principal ctc) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_upgrade_ctc',
        caller: controller,
        put_bytes: c_forwards([
            ctc.candid,
            Blob()
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}


Future<void> controller_take_away_cycles_transferrer_canisters(List<Principal> ctcs) async {
    Uint8List sponse = await cts.call(
        calltype: CallType.call,
        method_name: 'controller_take_away_cycles_transferrer_canisters',
        caller: controller,
        put_bytes: c_forwards([
            Vector.oftheList<PrincipalReference>(ctcs.map<PrincipalReference>((Principal p)=> p.candid).toList())
        ])
    );
    print(sponse);
    List<CandidType> cs = c_backwards(sponse);
    print(cs);

}




























Future<void> call_canister() async {
    late List<CandidType> cs;

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_size',

    // ));
    // print(cs);


    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_grow',
    //     put_bytes: c_forwards([
    //         Nat64(200),
    //     ])

    // ));
    // print(cs);

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_size',
    // ));
    // print(cs);


    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_write',
    //     put_bytes: c_forwards([
    //         Nat64(2),
    //         Blob([1,2,3]),
    //     ])

    // ));
    // print(cs);

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_size',
    // ));
    // print(cs);

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_bytes',
    // ));
    // Blob b = cs[0] as Blob;
    // // print(b.bytes);
    // print(b.length);

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_stable_read',
    //     put_bytes: c_forwards([
    //         Nat64(0),
    //         Nat64(20)
    //     ])

    // ));
    // print((cs[0] as Blob).bytes);

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_clear_file_hashes',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);




    // cs = c_backwards(await cts.call(
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

    // cs = c_backwards(await cts.call(
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

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_get_file_hashes',
    // ));
    // // print(cs);
    // for (Record r in (cs[0] as Vector).cast_vector<Record>()) {
    //     print(r[0]);
    // }


    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'upload_frontcode_file_chunks',
    //     put_bytes: c_forwards([
    //         Text('/hello'),
    //         Blob(utf8.encode('hello'))
    //     ])
    // ));
    // print(cs);

    // cs = c_backwards(await cts.call(
    //     calltype: CallType.call,
    //     method_name: 'public_get_file_hashes',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);

    cs = c_backwards(await cts.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'cycles_balance',
        put_bytes: c_forwards([])
    ));
    print(cs);

    // cs = c_backwards(await cts.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'sync_controllers',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);

    // cs = c_backwards(await cts.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'total_pages',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);

    cs = c_backwards(await cts.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'transfer_cycles',
        put_bytes: c_forwards([
            cts.principal.candid,
            Nat64(BigInt.from(3)),
            Record.oftheMap({
                'memo': Variant.oftheMap({
                    'text': Text('memo')
                })
            })
        ])
    ));
    print(cs);

    // cs = c_backwards(await cts.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'total_pages',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);


    // cs = c_backwards(await cts.call(
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
    
    
    cs = c_backwards(await cts.call(
        caller: controller,
        calltype: CallType.call,
        method_name: 'transfers',
        put_bytes: c_forwards([Nat64(BigInt.from(1))])
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

    cs = c_backwards(await cts.call(
        caller: controller2,
        calltype: CallType.call,
        method_name: 'transfers',
        put_bytes: c_forwards([Nat64(BigInt.from(1))])
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

    // cs = c_backwards(await cts.call(
    //     caller: controller,
    //     calltype: CallType.call,
    //     method_name: 'cycles_balance',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);


    cs = c_backwards(await cts.call(
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
                'canister_id': cts.principal.candid,
                'settings' : Record.oftheMap({
                    'controllers': Vector.oftheList<PrincipalReference>([
                        controller.principal.candid,
                        cts.principal.candid
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
                'canister_id': cts.principal.candid,
                'settings' : Record.oftheMap({
                    'controllers': Vector.oftheList<PrincipalReference>([
                        controller2.principal.candid,
                        cts.principal.candid
                    ])
                })
            })
        ])
    ));
    print(cs);


}



Future<void> put_frontcode_files(String main_or_test) async {
    Canister put_frontcode_on_the_canister = test_cts;
    Caller use_caller = controller;
    
    if (main_or_test == 'main') {
        put_frontcode_on_the_canister = cts;
        use_caller = thp4z_controller;
    } else if (main_or_test != 'test') {
        throw Exception('must be main or test');
    } 
    
    print('putting frontcode on the canister: ${put_frontcode_on_the_canister.principal}');
    print('confirm y/n');
    String confirm = stdin.readLineSync()!;
    if (confirm != 'y') { throw Exception('void-confirm'); }
    String build_web_dir_string = '$home_dir/frontcode/build/web';
    
    List<Future> upload_files_futures = [];
    
    await for (FileSystemEntity fse in Directory(build_web_dir_string).list(recursive: true, followLinks: false)) {
        if ( 
            await FileSystemEntity.isFile(fse.path) 
            //&& (fse.path.contains('main.dart.js') || fse.path.contains('main.dart.js'))
        ) {
            print(fse.path.replaceFirst(build_web_dir_string, ''));            
            
            String filename = fse.path.contains('/index.html') ? '/' : fse.path.replaceFirst(build_web_dir_string, '');
            
            String content_type = '';
            if (fse.path.substring(fse.path.length-5) == '.wasm') { content_type = 'application/wasm'; }
            
            List<int> file_bytes = gzip.encode(File(fse.path).readAsBytesSync());
            Iterable<List<int>> file_bytes_chunks = file_bytes.slices(1024*1024 + 1024*512);
            
            upload_files_futures.add(Future(()async{
                List<CandidType> cs = c_backwards(await put_frontcode_on_the_canister.call(
                    calltype: CallType.call,
                    method_name: 'controller_upload_file',
                    caller: use_caller,
                    put_bytes: c_forwards([
                        Record.oftheMap({
                            'filename': Text(filename),
                            'first_chunk': Blob(file_bytes_chunks.first),
                            'chunks': Nat32(file_bytes_chunks.length),
                            'content_type': Text(content_type),
                            'content_encoding': Text('gzip'),
                        }),
                    ])
                ));
                print('$filename: $cs');
                
                if (file_bytes_chunks.length > 1) {
                    List<Future> upload_chunks_futures = [];
                    for (int i = 1; i<file_bytes_chunks.length; i++) {
                        upload_chunks_futures.add(Future(()async{
                            List<CandidType> cschunk = c_backwards(await put_frontcode_on_the_canister.call(
                                calltype: CallType.call,
                                method_name: 'controller_upload_file_chunks',
                                caller: use_caller,
                                put_bytes: c_forwards([
                                    Text(filename),
                                    Nat32(i),
                                    Blob(file_bytes_chunks.elementAt(i)),
                                ])
                            ));
                            print('$filename : $i -> $cschunk');
                        }));
                    }
                    await Future.wait(upload_chunks_futures);
                }
            }));

        }
    }
    
    await Future.wait(upload_files_futures);
}




Future<void> clear_frontcode_files(String main_or_test) async {
    Canister clear_frontcode_on_the_canister = test_cts;
    Caller use_caller = controller;
    
    if (main_or_test == 'main') {
        clear_frontcode_on_the_canister = cts;
        use_caller = thp4z_controller;
    } else if (main_or_test != 'test') {
        throw Exception('must be main or test');
    } 
    
    print('clear frontcode on the canister: ${clear_frontcode_on_the_canister.principal}');
    print('confirm y/n');
    String confirm = stdin.readLineSync()!;
    if (confirm != 'y') {
        throw Exception('confirmation cancel');
    }
    
    List<CandidType> cs = c_backwards(await clear_frontcode_on_the_canister.call(
        calltype: CallType.call,
        method_name: 'controller_clear_files',
        caller: use_caller
    ));
    print(cs);
}


Future<void> clear_frontcode_file(String filename) async {
    Canister clear_file_on_the_canister = test_cts;
    print('clear frontcode file: ${filename} on the canister: ${clear_file_on_the_canister.principal}');
    print('confirm y/n');
    String confirm = stdin.readLineSync()!;
    if (confirm != 'y') {
        throw Exception('confirmation cancel');
    }
    
    List<CandidType> cs = c_backwards(await clear_file_on_the_canister.call(
        calltype: CallType.call,
        method_name: 'controller_clear_file',
        caller: controller,
        put_bytes: c_forwards([
            Text(filename)
        ])
    ));
    print(cs);
}

